//! Slide navigation, placeholder affordances and the persistent speaker notes pane.
use super::*;
use gpui_kit::component::{button::ButtonVariants, input::InputEvent};
use joydeck_engine::{Command, TextTarget};

impl Viewer {
    pub(super) fn rail_width(&self, w: &Window) -> f32 {
        if f32::from(w.viewport_size().width) < 1100. {
            180.
        } else {
            236.
        }
    }
    pub(super) fn slide_rail(&self, w: &Window, count: usize, cx: &mut Context<Self>) -> Div {
        let palette = chrome::palette(self.dark);
        let rail = self.rail_width(w);
        let width = rail - 42.;
        let height = self
            .deck
            .as_ref()
            .map_or(width * 9. / 16., |d| {
                width * d.info.height_pt as f32 / d.info.width_pt as f32
            })
            .clamp(48., 200.);
        div()
            .w(px(rail))
            .h_full()
            .flex_shrink_0()
            .border_r_1()
            .border_color(rgb(palette.border))
            .child(
                uniform_list(
                    "slide-list",
                    count,
                    cx.processor(move |s, range: std::ops::Range<usize>, _, cx| {
                        s.request_thumbnails(range.clone(), cx);
                        range
                            .map(|page| {
                                let title = s
                                    .deck
                                    .as_ref()
                                    .map(|d| d.info.slides[page].title.clone())
                                    .unwrap_or_default();
                                div()
                                    .id(page)
                                    .h(px(height + 28.))
                                    .pt_3()
                                    .pr_2()
                                    .flex()
                                    .items_start()
                                    .child(
                                        div()
                                            .w(px(25.))
                                            .pt_1()
                                            .text_center()
                                            .text_size(px(11.))
                                            .text_color(rgb(if s.page == page {
                                                palette.accent
                                            } else {
                                                palette.muted
                                            }))
                                            .child(format!("{}", page + 1)),
                                    )
                                    .child(
                                        div()
                                            .rounded(px(5.))
                                            .border(px(if s.page == page { 2. } else { 1. }))
                                            .border_color(rgb(if s.page == page {
                                                palette.accent
                                            } else {
                                                palette.border
                                            }))
                                            .child(
                                                Button::new(("slide", page))
                                                    .ghost()
                                                    .p_0()
                                                    .w(px(width + 6.))
                                                    .h(px(height + 6.))
                                                    .rounded(px(5.))
                                                    .border_0()
                                                    .accessibility_label(format!(
                                                        "幻灯片 {} {}",
                                                        page + 1,
                                                        title
                                                    ))
                                                    .tooltip(title)
                                                    .child(
                                                        div()
                                                            .w(px(width))
                                                            .h(px(height))
                                                            .bg(rgb(0xffffff))
                                                            .overflow_hidden()
                                                            .flex()
                                                            .items_center()
                                                            .justify_center()
                                                            .children(s.thumbnails.get(&page).map(
                                                                |i| {
                                                                    img(i.clone())
                                                                        .w_full()
                                                                        .h_full()
                                                                        .object_fit(
                                                                            ObjectFit::Contain,
                                                                        )
                                                                },
                                                            )),
                                                    )
                                                    .on_click(cx.listener(move |s, _, w, cx| {
                                                        if s.composing() {
                                                            return;
                                                        }
                                                        s.commit_notes(w, cx);
                                                        s.select(page, cx);
                                                        s.focus.focus(w, cx);
                                                    })),
                                            ),
                                    )
                            })
                            .collect::<Vec<_>>()
                    }),
                )
                .track_scroll(&self.scroll)
                .h_full(),
            )
    }
    pub(super) fn slide_sorter(&self, cx: &mut Context<Self>) -> AnyElement {
        let p = chrome::palette(self.dark);
        let count = self.deck.as_ref().map_or(0, |d| d.info.slides.len());
        div()
            .id("slide-sorter")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .p_6()
            .bg(rgb(p.canvas))
            .child(
                uniform_list(
                    "sorter-list",
                    count.div_ceil(3),
                    cx.processor(move |s, range: std::ops::Range<usize>, _, cx| {
                        let start = range.start * 3;
                        let end = (range.end * 3).min(count);
                        s.request_thumbnails(start..end, cx);
                        range
                            .map(|r| {
                                let mut row = div().h(px(205.)).flex().gap_6();
                                for page in r * 3..((r + 1) * 3).min(count) {
                                    row = row.child(
                                        div()
                                            .flex_1()
                                            .min_w_0()
                                            .flex()
                                            .flex_col()
                                            .gap_2()
                                            .child(
                                                Button::new(("sorter-slide", page))
                                                    .ghost()
                                                    .p_1()
                                                    .h(px(165.))
                                                    .w_full()
                                                    .border_2()
                                                    .border_color(rgb(if page == s.page {
                                                        p.accent
                                                    } else {
                                                        p.border
                                                    }))
                                                    .accessibility_label(format!(
                                                        "打开幻灯片 {}",
                                                        page + 1
                                                    ))
                                                    .child(
                                                        div()
                                                            .size_full()
                                                            .bg(rgb(0xffffff))
                                                            .flex()
                                                            .items_center()
                                                            .justify_center()
                                                            .children(s.thumbnails.get(&page).map(
                                                                |i| {
                                                                    img(i.clone())
                                                                        .max_w_full()
                                                                        .max_h_full()
                                                                        .object_fit(
                                                                            ObjectFit::Contain,
                                                                        )
                                                                },
                                                            )),
                                                    )
                                                    .on_click(cx.listener(move |s, _, w, cx| {
                                                        s.select(page, cx);
                                                        s.sorter = false;
                                                        s.focus.focus(w, cx);
                                                    })),
                                            )
                                            .child(
                                                div()
                                                    .text_size(px(12.))
                                                    .child(format!("{}", page + 1)),
                                            ),
                                    );
                                }
                                row
                            })
                            .collect::<Vec<_>>()
                    }),
                )
                .h_full(),
            )
            .into_any_element()
    }
    pub(super) fn placeholder_overlays(
        &self,
        scale: f32,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let Some(e) = &self.editor else { return vec![] };
        let p = e.session.model();
        let slide = &p.slides[self.page];
        let Some(ctx) = joydeck_model::resolve::Ctx::for_slide(p, slide) else {
            return vec![];
        };
        slide
            .shapes
            .iter()
            .filter(|s| {
                s.ph.is_some()
                    && s.text.as_ref().is_some_and(|t| t.text().is_empty())
                    && !e.session.selection().shapes.contains(&s.id)
            })
            .map(|s| {
                let b = joydeck_model::resolve::xfrm(&ctx, s);
                let id = s.id;
                let title = s.ph.as_ref().is_some_and(|p| {
                    matches!(
                        p.kind,
                        joydeck_model::PhType::Title | joydeck_model::PhType::CtrTitle
                    )
                });
                let label = if title {
                    "单击此处添加标题"
                } else if s
                    .ph
                    .as_ref()
                    .is_some_and(|p| p.kind == joydeck_model::PhType::SubTitle)
                {
                    "单击此处添加副标题"
                } else {
                    "单击此处添加文本"
                };
                Button::new(("placeholder", id.0))
                    .ghost()
                    .absolute()
                    .left(px(b.x as f32 * scale))
                    .top(px(b.y as f32 * scale))
                    .w(px(b.w as f32 * scale))
                    .h(px(b.h as f32 * scale))
                    .rounded_none()
                    .border_1()
                    .border_color(rgb(0xdddddd))
                    .accessibility_label(label)
                    .child(
                        div()
                            .size_full()
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_color(rgb(0x5c5c5c))
                            .text_size(px(if title { 44. } else { 24. } * scale))
                            .child(label),
                    )
                    .on_click(cx.listener(move |s, _, w, cx| {
                        if let Some(e) = &mut s.editor {
                            let _ = e.session.select(s.page, vec![id]);
                        }
                        s.begin_selected_text(w, cx);
                    }))
                    .into_any_element()
            })
            .collect()
    }
    pub(super) fn notes_pane(&self) -> Div {
        let p = chrome::palette(self.dark);
        div()
            .h(px(58.))
            .flex_shrink_0()
            .border_t_1()
            .border_color(rgb(p.border))
            .px_2()
            .pt_1()
            .bg(rgb(p.chrome))
            .child(
                Textarea::new(&self.notes_input)
                    .appearance(false)
                    .bordered(false)
                    .h(px(50.))
                    .w_full()
                    .aria_label("演讲备注")
                    .accessibility_id("speaker-notes"),
            )
    }
    pub(super) fn install_notes(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        self.notes_subscription =
            Some(
                cx.subscribe_in(&self.notes_input, w, |s, _, event: &InputEvent, w, cx| {
                    if !s.notes_syncing && matches!(event, InputEvent::Change | InputEvent::Blur) {
                        s.commit_notes(w, cx);
                    }
                }),
            );
    }
    pub(super) fn sync_notes(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        let value = self
            .editor
            .as_ref()
            .map(|e| e.session.model().slides[self.page].notes.text())
            .unwrap_or_default();
        if self.notes_input.read(cx).value().as_ref() != value
            && !self.notes_input.read(cx).focus_handle(cx).is_focused(w)
        {
            self.notes_syncing = true;
            self.notes_input
                .update(cx, |i, cx| i.set_value(value, w, cx));
            self.notes_syncing = false;
        }
    }
    pub(super) fn commit_notes(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        if self.notes_syncing
            || self
                .notes_input
                .update(cx, |s, cx| s.marked_text_range(w, cx).is_some())
        {
            return;
        }
        let Some(e) = &self.editor else { return };
        let value = self.notes_input.read(cx).value().to_string();
        let mut body = e.session.model().slides[self.page].notes.clone();
        if value == body.text() {
            return;
        }
        let end = body.text().len();
        if let Err(error) = joydeck_engine::text::replace(&mut body, 0..end, &value) {
            self.message = error;
            return;
        }
        self.dispatch(
            Command::SetText {
                target: TextTarget::Notes,
                body: Box::new(body),
            },
            w,
            cx,
        );
    }
    pub(super) fn save_as(&mut self, _: &SaveAs, w: &mut Window, cx: &mut Context<Self>) {
        if self.composing() || self.saving || self.loading || self.editor.is_none() {
            return;
        }
        self.commit_notes(w, cx);
        let base = self
            .source
            .as_ref()
            .and_then(|p| p.parent())
            .unwrap_or_else(|| std::path::Path::new("."));
        let name = self
            .source
            .as_ref()
            .and_then(|p| p.file_stem())
            .map(|s| format!("{} - 副本.joydeck", s.to_string_lossy()))
            .unwrap_or("演示文稿.joydeck".into());
        let rx = cx.prompt_for_new_path(base, Some(&name));
        let window = w.window_handle();
        let generation = self.generation;
        self.dialog_task = Some(cx.spawn(async move |view, cx| match rx.await {
            Ok(Ok(Some(mut path))) => {
                if path.extension().is_none() {
                    path.set_extension("joydeck");
                }
                let _ = window.update(cx, |_, w, cx| {
                    let _ = view.update(cx, |s, cx| {
                        if s.generation != generation {
                            return;
                        }
                        s.save_input.update(cx, |i, cx| {
                            i.set_value(path.to_string_lossy().to_string(), w, cx)
                        });
                        s.save(&Save, w, cx);
                        s.backstage = false;
                    });
                });
            }
            Ok(Ok(None)) => {
                let _ = view.update(cx, |s, cx| {
                    s.close_after_save = false;
                    s.pending_replace = None;
                    cx.notify();
                });
            }
            _ => {
                let _ = view.update(cx, |s, cx| {
                    s.message = "无法打开另存为对话框".into();
                    s.close_after_save = false;
                    s.pending_replace = None;
                    cx.notify();
                });
            }
        }));
    }
}
