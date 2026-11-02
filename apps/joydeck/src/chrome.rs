//! Main workspace composition and input routing.
use super::ui::*;
use super::*;
use gpui_kit::{
    assets::IconName as I,
    component::{Sizable, button::ButtonVariants},
};
impl Editor {
    pub(super) fn home_text(&mut self, op: &str, w: &mut Window, cx: &mut Context<Self>) {
        if ["left", "center", "right"].contains(&op) {
            self.finish_text(cx);
            if let Some((i, mut o)) = self.author_object() {
                o.text_align = op.into();
                self.command(Command::ReplaceObject(self.page, i, o), cx);
            }
            return;
        }
        if self.text_edit.is_none()
            && let Some(i) = self.selected
        {
            self.begin_text(i, w, cx);
            if let Some(t) = &mut self.text_edit {
                t.selection = 0..t.object.text.len();
            }
        }
        self.format_text(op, w, cx);
    }

    pub(super) fn file_dialog(&mut self, kind: &str, w: &mut Window, cx: &mut Context<Self>) {
        self.finish_text(cx);
        let kind = kind.to_owned();
        let window = w.window_handle();
        if kind == "open" || kind == "image" || kind == "replace-image" {
            let receiver = cx.prompt_for_paths(PathPromptOptions {
                files: true,
                directories: false,
                multiple: false,
                prompt: Some(
                    if kind.contains("image") {
                        "插入图片"
                    } else {
                        "打开演示文稿"
                    }
                    .into(),
                ),
            });
            cx.spawn(async move |view, cx| {
                if let Ok(Ok(Some(paths))) = receiver.await
                    && let Some(path) = paths.first()
                {
                    let path = path.clone();
                    let _ = window.update(cx, |_, w, cx| {
                        let _ = view.update(cx, |s, cx| {
                            if kind.contains("image") {
                                s.media_path.update(cx, |f, cx| {
                                    f.set_value(path.to_string_lossy().to_string(), w, cx)
                                });
                                s.import_media(None, kind == "replace-image", w, cx);
                            } else if path
                                .extension()
                                .is_some_and(|e| e.eq_ignore_ascii_case("pptx"))
                            {
                                s.delivery_fields[0].update(cx, |f, cx| {
                                    f.set_value(path.to_string_lossy().to_string(), w, cx)
                                });
                                s.delivery("import", w, cx);
                            } else {
                                s.path.update(cx, |f, cx| {
                                    f.set_value(path.to_string_lossy().to_string(), w, cx)
                                });
                                s.open(false, w, cx);
                            }
                        });
                    });
                }
            })
            .detach();
        } else {
            let ext = match kind.as_str() {
                "pptx" | "preserve" => "pptx",
                "pdf" | "pdf-notes" => "pdf",
                _ => "joydeck",
            };
            let suggestion = format!("演示文稿.{ext}");
            let directory = self
                .disk
                .as_ref()
                .and_then(|p| p.0.parent())
                .unwrap_or_else(|| std::path::Path::new("."));
            let receiver = cx.prompt_for_new_path(directory, Some(&suggestion));
            cx.spawn(async move |view, cx| {
                if let Ok(Ok(Some(path))) = receiver.await {
                    let _ = window.update(cx, |_, w, cx| {
                        let _ = view.update(cx, |s, cx| {
                            if kind == "save" || kind == "save-as" {
                                s.path.update(cx, |f, cx| {
                                    f.set_value(path.to_string_lossy().to_string(), w, cx)
                                });
                                s.save(&Save, w, cx);
                            } else {
                                s.delivery_fields[0].update(cx, |f, cx| {
                                    f.set_value(path.to_string_lossy().to_string(), w, cx)
                                });
                                s.delivery(&kind, w, cx);
                            }
                        });
                    });
                }
            })
            .detach();
        }
    }
    pub(super) fn render_workspace(
        &mut self,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        if !self.focus_initialized {
            self.focus.focus(w, cx);
            self.focus_initialized = true;
        }
        self.sync_document(cx);
        if self
            .control_service
            .as_ref()
            .is_some_and(|service| service.document_id != self.session.id)
        {
            self.control_service = None;
            self.message = "文稿已切换，本机控制已关闭".into();
        }
        if self.av_player.is_some()
            && (self.av_document_id != self.session.id
                || self
                    .av_slide
                    .as_ref()
                    .is_some_and(|id| id != &self.session.deck.slides[self.page].id)
                    && !self
                        .session
                        .deck
                        .playback
                        .clips
                        .get(self.clip_pick)
                        .is_some_and(|c| c.cross_slide))
        {
            self.av_player = None;
            self.av_image = None;
        }

        if self.doc_page != self.session.deck.slides[self.page].id {
            self.sync_doc_fields(w, cx);
        }
        if !w.is_window_active() && self.gesture.is_some() {
            self.cancel_gesture(&CancelGesture, w, cx);
        }
        self.request_render(cx);
        let pages = self.session.deck.slides.len();
        let layout = self.workspace_layout(w);
        let scale = self.canvas_scale(w);
        self.last_scale = scale;
        let surface = self.slide_surface(scale, cx);
        let inspector = if matches!(
            self.ribbon.as_str(),
            "图表"
                | "图示"
                | "动画"
                | "切换"
                | "幻灯片放映"
                | "审阅"
                | "媒体"
                | "输出"
                | "比较"
                | "维护"
        ) {
            Some(if matches!(self.ribbon.as_str(), "比较" | "维护") {
                self.maintenance_pane(layout.pane, cx)
            } else if matches!(self.ribbon.as_str(), "审阅" | "媒体" | "输出") {
                self.extended_pane(layout.pane, cx)
            } else {
                self.author_pane(layout.pane, cx)
            })
        } else if layout.pane > 0. {
            Some(self.inspector(layout.pane, cx))
        } else {
            None
        };
        let ribbon = if layout.compact {
            self.compact_ribbon(cx)
        } else {
            self.ribbon_content(cx)
        };
        div()
            .id("editor")
            .key_context("Editor")
            .track_focus(&self.focus)
            .size_full()
            .flex()
            .flex_col()
            .bg(rgb(PALETTE.canvas))
            .text_color(rgb(PALETTE.text))
            .text_size(px(13.))
            .on_key_down(cx.listener(|s, e: &KeyDownEvent, w, cx| {
                if s.table_key(e, w, cx) {
                    return;
                }
                if s.text_edit.is_none()
                    && s.focus.is_focused(w)
                    && (e.keystroke.modifiers.platform || e.keystroke.modifiers.control)
                {
                    match e.keystroke.key.as_str() {
                        "c" => {
                            s.copy_objects(cx);
                            cx.stop_propagation();
                            return;
                        }
                        "v" => {
                            s.paste_media(w, cx);
                            cx.stop_propagation();
                            return;
                        }
                        _ => {}
                    }
                }
                if s.text_edit.is_none()
                    && s.focus.is_focused(w)
                    && matches!(e.keystroke.key.as_str(), "delete" | "backspace")
                {
                    s.delete_selection(cx);
                    cx.stop_propagation();
                    return;
                }
                s.text_key(e, w, cx);
            }))
            .on_drop::<ExternalPaths>(cx.listener(|s, paths: &ExternalPaths, w, cx| {
                if let Some(path) = paths.paths().first() {
                    s.media_path.update(cx, |f, cx| {
                        f.set_value(path.to_string_lossy().to_string(), w, cx)
                    });
                    s.import_media(None, false, w, cx);
                }
            }))
            .on_action(cx.listener(|s, _: &CellNext, w, cx| s.cell_tab(false, w, cx)))
            .on_action(cx.listener(|s, _: &CellPrevious, w, cx| s.cell_tab(true, w, cx)))
            .on_action(cx.listener(|_, _: &NewDocument, _, cx| {
                let _ = super::open(cx);
            }))
            .on_action(cx.listener(|s, _: &NewSlide, _, cx| s.command(Command::AddPage, cx)))
            .on_action(cx.listener(|s, _: &ShowFirst, w, cx| s.delivery("show-first", w, cx)))
            .on_action(cx.listener(|s, _: &ShowCurrent, w, cx| s.delivery("show-current", w, cx)))
            .on_action(cx.listener(Self::undo))
            .on_action(cx.listener(Self::redo))
            .on_action(cx.listener(Self::save))
            .on_action(cx.listener(|s, _: &OpenFile, w, cx| s.file_dialog("open", w, cx)))
            .on_action(cx.listener(|s, _: &SaveAs, w, cx| s.file_dialog("save-as", w, cx)))
            .on_action(cx.listener(Self::close))
            .on_action(cx.listener(Self::cancel_gesture))
            .on_mouse_move(cx.listener(|s, e, _, cx| s.move_pointer(e, cx)))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|s, _, w, cx| {
                    s.end_table_resize(cx);
                    s.text_anchor = None;
                    s.end_gesture(cx);
                    s.page_drag = None;
                    s.sync_fields(w, cx);
                }),
            )
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(|s, _, w, cx| {
                    s.table_resize = None;
                    s.table_drag = false;
                    s.page_drag = None;
                    s.cancel_gesture(&CancelGesture, w, cx);
                    s.visual_revision += 1;
                    s.pending = None;
                    cx.notify();
                }),
            )
            .on_action(cx.listener(|s, _: &crate::Quit, w, cx| {
                cx.stop_propagation();
                s.close(&Close, w, cx);
            }))
            .child(self.document_header(w, cx))
            .child(self.ribbon_tabs(cx))
            .child(div().h(px(layout.ribbon)).flex_shrink_0().child(ribbon))
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .child(
                        div()
                            .w(px(layout.rail))
                            .flex_shrink_0()
                            .border_r_1()
                            .border_color(rgb(PALETTE.border))
                            .bg(rgb(PALETTE.chrome))
                            .flex()
                            .flex_col()
                            .child(
                                div()
                                    .h(px(44.))
                                    .flex_shrink_0()
                                    .px_3()
                                    .flex()
                                    .items_center()
                                    .justify_between()
                                    .child(section("幻灯片").pt_0())
                                    .child(tool("rail-add", "", I::Plus).on_click(
                                        cx.listener(|s, _, _, cx| s.command(Command::AddPage, cx)),
                                    )),
                            )
                            .child(
                                uniform_list(
                                    "edit-page-thumbnails",
                                    pages,
                                    cx.processor(move |s, range: std::ops::Range<usize>, _, cx| {
                                        s.request_thumbs(range.clone(), cx);
                                        range
                                            .map(|p| {
                                                let active = s.page_selection.contains(&p);
                                                let hidden = s.session.deck.slides[p].hidden;
                                                let aspect =
                                                    s.session.deck.width / s.session.deck.height;
                                                let thumb_width = layout.rail - 42.;
                                                let h = thumb_width / aspect;
                                                div()
                                                    .id(("page", p))
                                                    .h(px(h + 24.))
                                                    .px_3()
                                                    .py_3()
                                                    .cursor_pointer()
                                                    .flex()
                                                    .gap_2()
                                                    .child(
                                                        div()
                                                            .pt_1()
                                                            .w(px(14.))
                                                            .text_size(px(11.))
                                                            .text_color(rgb(if active {
                                                                ACCENT
                                                            } else {
                                                                PALETTE.muted
                                                            }))
                                                            .child(format!("{}", p + 1)),
                                                    )
                                                    .child(
                                                        div()
                                                            .flex_1()
                                                            .flex()
                                                            .flex_col()
                                                            .gap_2()
                                                            .child(
                                                                div()
                                                                    .h(px(h))
                                                                    .w_full()
                                                                    .bg(rgb(PALETTE.surface))
                                                                    .rounded_md()
                                                                    .border_2()
                                                                    .border_color(rgb(if active {
                                                                        ACCENT
                                                                    } else {
                                                                        PALETTE.border
                                                                    }))
                                                                    .flex()
                                                                    .items_center()
                                                                    .justify_center()
                                                                    .overflow_hidden()
                                                                    .children(
                                                                        s.thumbs.get(&p).map(|i| {
                                                                            img(i.clone())
                                                                                .w(px(h * aspect))
                                                                                .h(px(h))
                                                                        }),
                                                                    ),
                                                            )
                                                            .children(hidden.then(|| {
                                                                div()
                                                                    .absolute()
                                                                    .bottom_0()
                                                                    .right_0()
                                                                    .text_size(px(10.))
                                                                    .bg(rgb(PALETTE.chrome))
                                                                    .child("隐藏")
                                                            })),
                                                    )
                                                    .on_mouse_down(
                                                        MouseButton::Left,
                                                        cx.listener(
                                                            move |s, e: &MouseDownEvent, w, cx| {
                                                                s.finish_text(cx);
                                                                s.table_pick = None;
                                                                s.page_click(
                                                                    p,
                                                                    e.modifiers.shift,
                                                                    e.modifiers.control
                                                                        || e.modifiers.platform,
                                                                    cx,
                                                                );
                                                                s.page_drag = Some(s.page_ids());
                                                                s.focus.focus(w, cx);
                                                            },
                                                        ),
                                                    )
                                                    .on_mouse_up(
                                                        MouseButton::Left,
                                                        cx.listener(move |s, _, _, cx| {
                                                            if let Some(ids) = s.page_drag.take()
                                                                && !(ids.len() == 1 && ids[0] == p)
                                                            {
                                                                let target = if p >= ids[0] {
                                                                    p + 1
                                                                } else {
                                                                    p
                                                                };
                                                                s.command(
                                                                    Command::ReorderPages(
                                                                        ids, target,
                                                                    ),
                                                                    cx,
                                                                );
                                                                cx.stop_propagation();
                                                            }
                                                        }),
                                                    )
                                            })
                                            .collect::<Vec<_>>()
                                    }),
                                )
                                .flex_1()
                                .min_h_0(),
                            )
                            .child(
                                div().h(px(40.)).flex().justify_center().gap_1().children(
                                    [("up", "上移"), ("down", "下移"), ("delete", "删除")]
                                        .into_iter()
                                        .map(|(op, name)| {
                                            Button::new(SharedString::from(format!("slide-{op}")))
                                                .ghost()
                                                .small()
                                                .label(name)
                                                .on_click(cx.listener(move |s, _, _, cx| {
                                                    s.page_command(op, cx)
                                                }))
                                        }),
                                ),
                            ),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .child(
                                div()
                                    .id("stage")
                                    .flex_1()
                                    .min_h_0()
                                    .overflow_scroll()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .p(px(layout.padding))
                                    .bg(rgb(PALETTE.canvas))
                                    .child(surface),
                            )
                            .children((!self.show_notes).then(|| {
                                div()
                                    .id("notes-collapsed")
                                    .h(px(32.))
                                    .flex_shrink_0()
                                    .px_3()
                                    .bg(rgb(PALETTE.chrome))
                                    .border_t_1()
                                    .border_color(rgb(PALETTE.border))
                                    .flex()
                                    .items_center()
                                    .text_size(px(12.))
                                    .text_color(rgb(PALETTE.muted))
                                    .child("单击此处添加备注")
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(|s, _, w, cx| {
                                            s.show_notes = true;
                                            s.sync_doc_fields(w, cx);
                                            cx.notify();
                                        }),
                                    )
                            }))
                            .children(self.show_notes.then(|| {
                                div()
                                    .h(px(layout.notes))
                                    .flex_shrink_0()
                                    .px_5()
                                    .py_3()
                                    .bg(rgb(PALETTE.surface))
                                    .border_t_1()
                                    .border_color(rgb(PALETTE.border))
                                    .flex()
                                    .gap_4()
                                    .child(label("演讲者备注"))
                                    .child(
                                        div().flex_1().child(
                                            Textarea::new(&self.notes_input)
                                                .aria_label("演讲者备注")
                                                .accessibility_id("slide-notes"),
                                        ),
                                    )
                                    .child(
                                        Button::new("notes")
                                            .small()
                                            .label("保存备注")
                                            .on_click(cx.listener(|s, _, _, cx| s.notes(cx))),
                                    )
                            })),
                    )
                    .children(inspector),
            )
            .child(
                div()
                    .h(px(joyoffice_ui::metrics::STATUS))
                    .flex_shrink_0()
                    .px_4()
                    .flex()
                    .items_center()
                    .gap_4()
                    .border_t_1()
                    .border_color(rgb(PALETTE.border))
                    .bg(rgb(PALETTE.surface))
                    .text_size(px(11.))
                    .text_color(rgb(PALETTE.muted))
                    .child(format!("幻灯片 {} / {}", self.page + 1, pages))
                    .child(div().flex_1().truncate().child(self.message.clone()))
                    .child(
                        Button::new("notes-toggle")
                            .ghost()
                            .small()
                            .label("备注")
                            .on_click(cx.listener(|s, _, w, cx| {
                                s.show_notes = !s.show_notes;
                                s.sync_doc_fields(w, cx);
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("zoom-out")
                            .ghost()
                            .small()
                            .icon(I::Minus)
                            .on_click(cx.listener(|s, _, _, cx| {
                                s.zoom = (s.zoom / 1.15).max(0.25);
                                cx.notify();
                            })),
                    )
                    .child(format!("{}%", (scale * 100.).round() as u32))
                    .child(
                        Button::new("zoom-in")
                            .ghost()
                            .small()
                            .icon(I::Plus)
                            .on_click(cx.listener(|s, _, _, cx| {
                                s.zoom = (s.zoom * 1.15).min(3.);
                                cx.notify();
                            })),
                    )
                    .child(
                        tool("zoom-fit", "适应窗口", I::Maximize).on_click(cx.listener(
                            |s, _, _, cx| {
                                s.zoom = 1.;
                                cx.notify();
                            },
                        )),
                    ),
            )
            .into_any_element()
    }
}
