//! Native review and comparison controls; edits use the shared engine transaction.
use super::*;
use gpui_kit::component::Sizable;
use joydeck_engine::{Command, ReviewAction};

impl Viewer {
    pub(super) fn add_comment(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        let shape = self
            .editor
            .as_ref()
            .and_then(|e| e.session.selection().shapes.first().copied());
        self.fields_dialog(
            self.t("review.add_comment_title"),
            vec![
                (
                    self.t("review.field_author"),
                    self.t("review.default_author").into(),
                ),
                (self.t("review.field_comment"), String::new()),
            ],
            w,
            cx,
            move |s, v, w, cx| {
                if v[0].trim().is_empty() || v[1].trim().is_empty() {
                    return Err(s.t("review.err_comment_required").into());
                }
                s.dispatch(
                    Command::Review {
                        action: ReviewAction::Add {
                            id: None,
                            author: v[0].clone(),
                            text: v[1].clone(),
                            shape,
                        },
                    },
                    w,
                    cx,
                );
                Ok(())
            },
        );
    }
    pub(super) fn accessibility_dialog(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        let Some(shape) = self.editor.as_ref().and_then(|e| {
            e.session
                .selection()
                .shapes
                .first()
                .and_then(|id| e.session.model().slides[self.page].shape(*id))
        }) else {
            return;
        };
        self.fields_dialog(
            self.t("review.alt_text_title"),
            vec![
                (self.t("review.field_description"), shape.descr.clone()),
                (
                    self.t("review.field_decorative"),
                    if shape.decorative {
                        self.t("common.yes")
                    } else {
                        self.t("common.no")
                    }
                    .into(),
                ),
            ],
            w,
            cx,
            |s, v, w, cx| {
                let raw = v[1].trim();
                let decorative = match raw.to_ascii_lowercase().as_str() {
                    "是" | "yes" | "y" | "true" | "1" => true,
                    "否" | "no" | "n" | "false" | "0" => false,
                    _ => return Err(s.t("review.err_decorative").into()),
                };
                s.dispatch(
                    Command::Review {
                        action: ReviewAction::Accessibility {
                            description: v[0].clone(),
                            decorative,
                        },
                    },
                    w,
                    cx,
                );
                Ok(())
            },
        );
    }
    pub(super) fn comments_dialog(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        let Some(e) = &self.editor else { return };
        let comments = e.session.model().slides[self.page].comments.clone();
        let generation = self.generation;
        let revision = e.session.revision();
        let page = self.page;
        let locale = self.locale;
        let view = cx.entity().downgrade();
        w.open_dialog(cx, move |d, _, _| {
            let t = |key: &'static str| crate::l10n::t(locale, key);
            let mut list = div()
                .id("comment-list")
                .flex()
                .flex_col()
                .max_h(px(430.))
                .overflow_y_scroll()
                .gap_3();
            if comments.is_empty() {
                list = list.child(t("review.no_comments"));
            }
            for (index, comment) in comments.iter().enumerate() {
                let mut item = div().flex().flex_col().gap_1().child(format!(
                    "{}{}：{}",
                    if comment.resolved {
                        t("review.resolved_prefix")
                    } else {
                        ""
                    },
                    comment.author,
                    comment.text
                ));
                for reply in &comment.replies {
                    item = item.child(format!("↳ {}：{}", reply.author, reply.text));
                }
                let resolve_view = view.clone();
                let reply_view = view.clone();
                let resolved = comment.resolved;
                item = item.child(
                    div()
                        .flex()
                        .gap_2()
                        .child(
                            Button::new(("resolve-comment", index))
                                .xsmall()
                                .label(if resolved {
                                    t("review.reopen")
                                } else {
                                    t("review.resolve")
                                })
                                .on_click(move |_, w, cx| {
                                    let _ = resolve_view.update(cx, |s, cx| {
                                        if s.generation == generation
                                            && s.page == page
                                            && s.editor
                                                .as_ref()
                                                .is_some_and(|e| e.session.revision() == revision)
                                        {
                                            s.dispatch(
                                                Command::Review {
                                                    action: ReviewAction::Resolve {
                                                        index,
                                                        resolved: !resolved,
                                                    },
                                                },
                                                w,
                                                cx,
                                            );
                                            w.close_dialog(cx);
                                        }
                                    });
                                }),
                        )
                        .child(
                            Button::new(("reply-comment", index))
                                .xsmall()
                                .label(t("review.reply_btn"))
                                .on_click(move |_, w, cx| {
                                    let _ = reply_view.update(cx, |s, cx| {
                                        if s.generation != generation
                                            || s.page != page
                                            || s.editor
                                                .as_ref()
                                                .is_none_or(|e| e.session.revision() != revision)
                                        {
                                            return;
                                        }
                                        w.close_dialog(cx);
                                        s.fields_dialog(
                                            s.t("review.reply_title"),
                                            vec![
                                                (
                                                    s.t("review.field_author"),
                                                    s.t("review.default_author").into(),
                                                ),
                                                (s.t("review.field_reply"), String::new()),
                                            ],
                                            w,
                                            cx,
                                            move |s, v, w, cx| {
                                                if v[0].trim().is_empty() || v[1].trim().is_empty()
                                                {
                                                    return Err(s
                                                        .t("review.err_reply_required")
                                                        .into());
                                                }
                                                s.dispatch(
                                                    Command::Review {
                                                        action: ReviewAction::Reply {
                                                            index,
                                                            author: v[0].clone(),
                                                            text: v[1].clone(),
                                                        },
                                                    },
                                                    w,
                                                    cx,
                                                );
                                                Ok(())
                                            },
                                        );
                                    });
                                }),
                        ),
                );
                list = list.child(item);
            }
            d.title(t("review.comments_title")).child(list)
        });
    }
    pub(super) fn compare_file(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        if self.composing() || self.loading {
            return;
        }
        self.end_inline(cx);
        let Some(editor) = &self.editor else { return };
        let model = editor.session.model_snapshot();
        let generation = self.generation;
        let revision = editor.session.revision();
        let receiver = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some(self.t("review.compare_prompt").into()),
        });
        self.compare_task = Some(cx.spawn_in(w, async move |view, cx| {
            if let Ok(Ok(Some(paths))) = receiver.await
                && let Some(path) = paths.into_iter().next()
            {
                let result = cx
                    .background_executor()
                    .spawn(async move {
                        let reference = joydeck_shell::Document::open(&path, &Default::default())?;
                        joydeck_shell::comparison::Comparison::from_model(&model, reference)
                    })
                    .await;
                let _ = view.update_in(cx, |s, w, cx| {
                    if s.generation != generation
                        || s.editor
                            .as_ref()
                            .is_none_or(|e| e.session.revision() != revision)
                    {
                        return;
                    }
                    match result {
                        Ok(comparison) => {
                            s.comparison = Some(comparison);
                            s.compare_dialog(w, cx);
                        }
                        Err(error) => {
                            s.message = error;
                            cx.notify();
                        }
                    }
                });
            }
        }));
    }
    fn compare_dialog(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        let Some(comparison) = &self.comparison else {
            return;
        };
        let report = comparison.report.clone();
        let locale = self.locale;
        let view = cx.entity().downgrade();
        w.open_dialog(cx, move |d, _, _| {
            let t = |key: &'static str| crate::l10n::t(locale, key);
            let mut list = div()
                .id("comparison-differences")
                .flex()
                .flex_col()
                .gap_2()
                .max_h(px(460.))
                .overflow_y_scroll();
            if report.differences.is_empty() {
                list = list.child(t("review.compare_no_diff"));
            }
            for (index, diff) in report.differences.iter().enumerate() {
                let view = view.clone();
                let base = report.base_sha256.clone();
                let reference = report.reference_sha256.clone();
                let page = diff.page;
                let object = diff.object;
                let page_s = diff.page.to_string();
                let obj_s = diff
                    .object
                    .map(|id| id.0.to_string())
                    .unwrap_or_else(|| "—".into());
                let kind_s = match diff.kind {
                    "added" => t("review.compare_added"),
                    "deleted" => t("review.compare_deleted"),
                    "changed" => t("review.compare_changed"),
                    _ => t("review.compare_other"),
                };
                list = list.child(
                    div()
                        .flex()
                        .gap_2()
                        .items_center()
                        .child(crate::l10n::tf(
                            locale,
                            "review.compare_row",
                            &[("page", &page_s), ("object", &obj_s), ("kind", kind_s)],
                        ))
                        .child(
                            Button::new(("accept-comparison", index))
                                .xsmall()
                                .label(t("review.compare_accept"))
                                .disabled(!diff.acceptable)
                                .on_click(move |_, w, cx| {
                                    let _ = view.update(cx, |s, cx| {
                                        let result = s
                                            .comparison
                                            .as_ref()
                                            .zip(s.editor.as_ref())
                                            .ok_or_else(|| {
                                                s.t("review.compare_unavailable").to_owned()
                                            })
                                            .and_then(|(comparison, editor)| {
                                                comparison.command_for_model(
                                                    editor.session.model(),
                                                    &base,
                                                    &reference,
                                                    page,
                                                    object
                                                        .ok_or(s.t("review.compare_no_object"))?,
                                                )
                                            });
                                        match result {
                                            Ok(command) => {
                                                s.dispatch(command, w, cx);
                                                s.comparison = None;
                                                w.close_dialog(cx);
                                            }
                                            Err(error) => {
                                                s.message = error;
                                                cx.notify();
                                            }
                                        }
                                    });
                                }),
                        ),
                );
            }
            d.title(t("review.compare_title")).child(list)
        });
    }
}
