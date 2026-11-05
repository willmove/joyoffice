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
            "新增批注",
            vec![("作者", "作者".into()), ("批注", String::new())],
            w,
            cx,
            move |s, v, w, cx| {
                if v[0].trim().is_empty() || v[1].trim().is_empty() {
                    return Err("请填写作者和批注".into());
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
            "替代文字",
            vec![
                ("说明", shape.descr.clone()),
                (
                    "装饰对象（是／否）",
                    if shape.decorative { "是" } else { "否" }.into(),
                ),
            ],
            w,
            cx,
            |s, v, w, cx| {
                let decorative = match v[1].trim() {
                    "是" => true,
                    "否" => false,
                    _ => return Err("装饰对象请填写是或否".into()),
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
        let view = cx.entity().downgrade();
        w.open_dialog(cx, move |d, _, _| {
            let mut list = div()
                .id("comment-list")
                .flex()
                .flex_col()
                .max_h(px(430.))
                .overflow_y_scroll()
                .gap_3();
            if comments.is_empty() {
                list = list.child("当前页没有批注");
            }
            for (index, comment) in comments.iter().enumerate() {
                let mut item = div().flex().flex_col().gap_1().child(format!(
                    "{}{}：{}",
                    if comment.resolved {
                        "已解决 · "
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
                                .label(if resolved { "重新打开" } else { "解决" })
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
                                .label("回复…")
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
                                            "回复批注",
                                            vec![("作者", "作者".into()), ("回复", String::new())],
                                            w,
                                            cx,
                                            move |s, v, w, cx| {
                                                if v[0].trim().is_empty() || v[1].trim().is_empty()
                                                {
                                                    return Err("请填写作者和回复".into());
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
            d.title("当前页批注").child(list)
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
            prompt: Some("选择比较文稿".into()),
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
        let view = cx.entity().downgrade();
        w.open_dialog(cx, move |d, _, _| {
            let mut list = div()
                .id("comparison-differences")
                .flex()
                .flex_col()
                .gap_2()
                .max_h(px(460.))
                .overflow_y_scroll();
            if report.differences.is_empty() {
                list = list.child("未发现文稿内容差异");
            }
            for (index, diff) in report.differences.iter().enumerate() {
                let view = view.clone();
                let base = report.base_sha256.clone();
                let reference = report.reference_sha256.clone();
                let page = diff.page;
                let object = diff.object;
                list = list.child(
                    div()
                        .flex()
                        .gap_2()
                        .items_center()
                        .child(format!(
                            "第 {} 页 · 对象 {} · {}",
                            diff.page,
                            diff.object
                                .map(|id| id.0.to_string())
                                .unwrap_or_else(|| "—".into()),
                            match diff.kind {
                                "added" => "新增",
                                "deleted" => "删除",
                                "changed" => "修改",
                                _ => "页面或文稿属性（仅报告）",
                            }
                        ))
                        .child(
                            Button::new(("accept-comparison", index))
                                .xsmall()
                                .label("接受")
                                .disabled(!diff.acceptable)
                                .on_click(move |_, w, cx| {
                                    let _ = view.update(cx, |s, cx| {
                                        let result = s
                                            .comparison
                                            .as_ref()
                                            .zip(s.editor.as_ref())
                                            .ok_or("比较结果不可用".into())
                                            .and_then(|(comparison, editor)| {
                                                comparison.command_for_model(
                                                    editor.session.model(),
                                                    &base,
                                                    &reference,
                                                    page,
                                                    object.ok_or("对象不存在")?,
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
            d.title("比较文稿 · 接受一项后请重新比较").child(list)
        });
    }
}
