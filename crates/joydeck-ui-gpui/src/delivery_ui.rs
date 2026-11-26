//! Snapshot delivery controls. All encoding, printing and file publication run off the UI thread.
use super::*;
use joydeck_shell::output::{Options, PageLayout};
impl Viewer {
    pub(super) fn delivery_toolbar(&self, cx: &mut Context<Self>) -> Div {
        let mut row = div()
            .min_h(px(40.))
            .flex()
            .flex_wrap()
            .items_center()
            .gap_1()
            .px_2();
        for (id, label, kind, layout) in [
            ("deliver-pdf", "PDF", "pdf", PageLayout::Slides),
            (
                "deliver-notes-pdf",
                self.t("delivery.notes_pdf"),
                "pdf",
                PageLayout::Notes,
            ),
            (
                "deliver-handouts-pdf",
                self.t("delivery.handouts_pdf"),
                "pdf",
                PageLayout::Handouts { per_page: 3 },
            ),
            ("deliver-png", "PNG", "png", PageLayout::Slides),
            ("deliver-jpeg", "JPEG", "jpeg", PageLayout::Slides),
            (
                "deliver-gif",
                self.t("delivery.animated_gif"),
                "gif",
                PageLayout::Slides,
            ),
            (
                "deliver-mp4",
                self.t("delivery.static_mp4"),
                "mp4",
                PageLayout::Slides,
            ),
            (
                "deliver-outline",
                self.t("delivery.outline"),
                "outline",
                PageLayout::Slides,
            ),
            (
                "deliver-vtt",
                self.t("delivery.vtt"),
                "vtt",
                PageLayout::Slides,
            ),
            (
                "deliver-print",
                self.t("delivery.print"),
                "print",
                PageLayout::Slides,
            ),
        ] {
            row = row.child(
                Button::new(id)
                    .label(label)
                    .disabled(self.editor.is_none() || self.delivery_busy)
                    .on_click(cx.listener(move |s, _, w, cx| s.delivery_form(kind, layout, w, cx))),
            );
        }
        row.child(
            Button::new("cancel-output")
                .label(self.t("delivery.cancel"))
                .disabled(!self.delivery_busy)
                .on_click(cx.listener(|s, _, _, cx| {
                    s.delivery_cancel.cancel();
                    s.message = s.t("delivery.cancelling").into();
                    cx.notify();
                })),
        )
    }
    pub(super) fn delivery_form(
        &mut self,
        kind: &'static str,
        layout: PageLayout,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.editor.is_none() || self.delivery_busy {
            return;
        }
        self.end_inline(cx);
        let locale = self.locale;
        let path = cx.new(|cx| {
            let mut s = InputState::new(w, cx);
            s.set_value("", w, cx);
            s.placeholder(crate::l10n::t(locale, "delivery.path_placeholder"))
        });
        let pages = cx.new(|cx| {
            let mut s = InputState::new(w, cx);
            s.set_value("all", w, cx);
            s
        });
        let density = cx.new(|cx| {
            let mut s = InputState::new(w, cx);
            s.set_value("144", w, cx);
            s
        });
        let generation = self.generation;
        let error = cx.new(|_| String::new());
        let view = cx.entity().downgrade();
        let title = if kind == "print" {
            self.t("delivery.print_title")
        } else {
            self.t("delivery.export_title")
        };
        w.open_dialog(cx, move |dialog, _, cx| {
            let t = |key: &'static str| crate::l10n::t(locale, key);
            let mut content = div()
                .id("output-fields")
                .max_h(px(340.))
                .overflow_y_scroll()
                .flex()
                .flex_col()
                .gap_2();
            if kind != "print" {
                content = content
                    .child(if ["png", "jpeg"].contains(&kind) {
                        t("delivery.dir_label")
                    } else {
                        t("delivery.file_label")
                    })
                    .child(Input::new(&path));
            }
            content = content
                .child(t("delivery.pages_hint"))
                .child(Input::new(&pages))
                .child(t("delivery.dpi_hint"))
                .child(Input::new(&density))
                .child(
                    div()
                        .text_color(rgb(0xc13d36))
                        .child(error.read(cx).clone()),
                );
            let (view, path, pages, density) =
                (view.clone(), path.clone(), pages.clone(), density.clone());
            let error = error.clone();
            dialog.title(title).child(content).child(
                Button::new("deliver")
                    .label(if kind == "print" {
                        t("delivery.open_print_dialog")
                    } else {
                        t("delivery.export_btn")
                    })
                    .on_click(move |_, w, cx| {
                        let _ = view.update(cx, |s, cx| {
                            if s.generation != generation {
                                s.message = s.t("delivery.doc_switched").into();
                                w.close_dialog(cx);
                                return;
                            }
                            for input in [&path, &pages, &density] {
                                if input.update(cx, |s, cx| s.marked_text_range(w, cx).is_some()) {
                                    s.message = s.t("delivery.finish_ime").into();
                                    cx.notify();
                                    return;
                                }
                            }
                            let range = pages.read(cx).value().to_string();
                            let destination = path.read(cx).value().to_string();
                            let parsed = (|| {
                                let selected = if range.trim() == "all" {
                                    None
                                } else {
                                    Some(
                                        range
                                            .split(',')
                                            .map(|x| {
                                                x.trim()
                                                    .parse::<usize>()
                                                    .ok()
                                                    .and_then(|p| p.checked_sub(1))
                                                    .ok_or(s.t("delivery.err_page_one"))
                                            })
                                            .collect::<std::result::Result<Vec<_>, _>>()?,
                                    )
                                };
                                let dpi: f64 = density
                                    .read(cx)
                                    .value()
                                    .parse()
                                    .map_err(|_| s.t("delivery.err_dpi_invalid"))?;
                                if !dpi.is_finite() || !(36.0..=300.).contains(&dpi) {
                                    return Err(s.t("delivery.err_dpi_range"));
                                }
                                if selected.as_ref().is_some_and(|pages: &Vec<usize>| {
                                    pages.is_empty()
                                        || pages.len() > 500
                                        || pages.iter().any(|p| {
                                            *p >= s
                                                .editor
                                                .as_ref()
                                                .map_or(0, |e| e.session.model().slides.len())
                                        })
                                }) {
                                    return Err(s.t("delivery.err_pages_range"));
                                }
                                if kind != "print" && destination.trim().is_empty() {
                                    return Err(s.t("delivery.err_path_empty"));
                                }
                                Ok::<_, &str>(Options {
                                    format: kind.into(),
                                    pages: selected,
                                    dpi,
                                    layout,
                                    ..Default::default()
                                })
                            })();
                            match parsed {
                                Ok(options) => {
                                    w.close_dialog(cx);
                                    s.deliver(destination.into(), options, cx);
                                }
                                Err(e) => {
                                    s.message = e.into();
                                    error.update(cx, |value, _| *value = e.into());
                                    w.refresh();
                                    cx.notify();
                                }
                            }
                        });
                    }),
            )
        });
    }
    fn deliver(&mut self, path: PathBuf, options: Options, cx: &mut Context<Self>) {
        let Some(editor) = &self.editor else { return };
        if self.delivery_busy {
            return;
        }
        let model = editor.session.model_snapshot();
        let revision = editor.session.revision();
        let generation = self.generation;
        let locale = self.locale;
        self.delivery_cancel = Default::default();
        let cancel = self.delivery_cancel.clone();
        self.delivery_busy = true;
        self.message = self.t("delivery.running").into();
        self.delivery_task = Some(cx.spawn(async move |view, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    let doc = joydeck_shell::Document::from_model(
                        "output snapshot".into(),
                        (*model).clone(),
                    )?;
                    if options.format == "print" {
                        joydeck_shell::output::print(&doc, &options, &cancel).map(|summary| {
                            crate::l10n::tf(
                                locale,
                                "delivery.print_result",
                                &[("summary", &summary)],
                            )
                        })
                    } else {
                        joydeck_shell::output::export(&doc, &path, &options, &cancel).map(|r| {
                            let pages_s = r.pages.len().to_string();
                            let files_s = r.files.to_string();
                            let bytes_s = r.bytes.to_string();
                            crate::l10n::tf(
                                locale,
                                "delivery.done",
                                &[
                                    ("pages", &pages_s),
                                    ("files", &files_s),
                                    ("bytes", &bytes_s),
                                ],
                            )
                        })
                    }
                })
                .await;
            let _ = view.update(cx, |s, cx| {
                s.delivery_busy = false;
                if generation == s.generation {
                    s.message =
                        result.unwrap_or_else(|e| s.tf("delivery.failed", &[("error", &e)]));
                    eprintln!("R6 delivery revision={revision} {}", s.message);
                    cx.notify();
                }
            });
        }));
        cx.notify();
    }
}
