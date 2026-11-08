//! Native GPUI input binding adapted from JoyOffice input_ui; no parallel text editor.
use super::*;
use gpui::{
    ElementInputHandler, EntityInputHandler, MouseButton, Pixels, Point, UTF16Selection, canvas,
    fill, point, rgba,
};
use joyword_shell::editing::utf16_bytes;
use std::{ops::Range, time::Duration};

pub(super) struct InputBridge {
    owner: gpui::WeakEntity<JoyWord>,
    document: u64,
}
impl InputBridge {
    fn update<T>(
        &self,
        cx: &mut Context<Self>,
        f: impl FnOnce(&mut JoyWord, &mut Context<JoyWord>) -> T,
    ) -> Option<T> {
        self.owner
            .update(cx, |view, cx| {
                if view.input_document() == Some(self.document)
                    && !view.canvas.loading
                    && !view.confirm_discard
                    && view.replace_confirm.is_none()
                    && !view.file_page
                {
                    Some(f(view, cx))
                } else {
                    None
                }
            })
            .ok()
            .flatten()
    }
}
impl EntityInputHandler for InputBridge {
    fn text_for_range(
        &mut self,
        range: Range<usize>,
        adjusted: &mut Option<Range<usize>>,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<String> {
        *adjusted = None;
        let text = self.update(cx, |view, _| {
            view.canvas.reader.as_ref()?.input_context().map(|c| c.text)
        })??;
        let bytes = utf16_bytes(&text, range.clone()).ok()?;
        *adjusted = Some(range);
        Some(text.get(bytes)?.to_string())
    }
    fn selected_text_range(
        &mut self,
        _: bool,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        self.update(cx, |v, _| {
            let context = v.canvas.reader.as_ref()?.input_context();
            Some(context.map_or(
                UTF16Selection {
                    range: 0..0,
                    reversed: false,
                },
                |c| UTF16Selection {
                    range: c.selection,
                    reversed: c.reversed,
                },
            ))
        })?
    }
    fn marked_text_range(&self, _: &mut Window, cx: &mut Context<Self>) -> Option<Range<usize>> {
        self.update(cx, |v, _| v.canvas.reader.as_ref()?.marked_range())?
    }
    fn unmark_text(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.update(cx, |v, cx| {
            v.apply_input(window, cx, |r| r.commit_composition())
        });
    }
    fn replace_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.update(cx, |v, cx| {
            v.apply_input(window, cx, |r| r.replace_text(range, text))
        });
    }
    fn replace_and_mark_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        selected: Option<Range<usize>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.update(cx, |v, cx| {
            v.apply_input(window, cx, |r| r.mark_text(range, text, selected))
        });
    }
    fn bounds_for_range(
        &mut self,
        range: Range<usize>,
        _: Bounds<Pixels>,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        self.update(cx, |v, _| {
            let reader = v.canvas.reader.as_ref()?;
            let pos = reader
                .native_positions(range)
                .map(|p| p.0)
                .unwrap_or_else(|_| reader.engine().sel.focus.clone());
            v.caret_bounds(&pos)
        })?
    }
    fn character_index_for_point(
        &mut self,
        point: Point<Pixels>,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<usize> {
        self.update(cx, |v, _| {
            v.canvas
                .reader
                .as_ref()?
                .native_index(&v.hit_position(point)?)
        })?
    }
}
impl JoyWord {
    fn input_document(&self) -> Option<u64> {
        self.canvas
            .reader
            .as_ref()
            .map(|r| r.engine().document_id())
    }
    pub(super) fn install_input(&mut self, document: u64, cx: &mut Context<Self>) {
        self.gesture = None;
        self.asset_task = None;
        self.format_state = None;
        let owner = cx.weak_entity();
        self.input = Some(cx.new(|_| InputBridge { owner, document }));
        self.page_bounds.clear();
        self.dragging = false;
    }
    pub(super) fn start_blink(&mut self, cx: &mut Context<Self>) {
        self.blink = Some(cx.spawn(async move |view, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(500))
                    .await;
                if view
                    .update(cx, |v, cx| {
                        if let Some(reader) = v.canvas.reader.as_mut() {
                            let before = reader.engine().rev();
                            reader.recovery_tick();
                            if reader.engine().rev() != before {
                                let document = reader.engine().document_id();
                                v.invalidate_document(cx);
                                v.install_input(document, cx);
                            }
                        }
                        v.caret_on = !v.caret_on;
                        cx.notify();
                    })
                    .is_err()
                {
                    break;
                }
            }
        }));
    }
    pub(super) fn apply_input(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
        f: impl FnOnce(&mut joyword_shell::reader::Reader) -> Result<(), String>,
    ) {
        if self.canvas.loading
            || self.confirm_discard
            || self.replace_confirm.is_some()
            || self.file_page
        {
            return;
        }
        let Some(reader) = self.canvas.reader.as_mut() else {
            return;
        };
        let started = perf::start();
        let before = reader.presentation_revision();
        let identity = reader.engine().document_id();
        let result = f(reader);
        self.format_state = None;
        let changed = reader.presentation_revision() != before;
        let new_identity = reader.engine().document_id();
        let layout_ms = reader.summary.layout_ms;
        if identity != new_identity {
            self.install_input(new_identity, cx);
        }
        if changed {
            self.invalidate_document(cx);
            self.canvas.input_paint = started;
            perf::sample("edit_callback", started, 0);
            if started.is_some() {
                perf::elapsed("edit_layout", layout_ms, 0);
            }
        }
        match result {
            Ok(()) => self.shell.notice = None,
            Err(e) => self.shell.notice = Some(e),
        }
        self.caret_on = true;
        self.reveal_input = true;
        window.invalidate_character_coordinates();
        cx.notify();
    }
    pub(super) fn cancel_preedit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.gesture.take().is_some() {
            self.dragging = false;
            cx.notify();
        }
        if let Some(reader) = self.canvas.reader.as_mut()
            && reader.cancel_composition()
        {
            self.format_state = None;
            self.invalidate_document(cx);
            window.invalidate_character_coordinates();
            cx.notify();
        }
    }
    pub(super) fn invalidate_document(&mut self, cx: &mut Context<Self>) {
        if let Some(epoch) = self.canvas.epoch.checked_add(1) {
            self.canvas.epoch = epoch;
            if !self.canvas.refresh_changed_page(cx) {
                self.canvas.invalidate_scene();
                self.clear_images(cx);
            }
        } else {
            self.shell.notice = Some("Document generation exhausted".into());
        }
    }
    pub(super) fn engine_command(
        &mut self,
        id: &str,
        args: &serde_json::Value,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.apply_input(window, cx, |reader| reader.edit_command(id, args));
    }
    pub(super) fn document_key(
        &mut self,
        event: &gpui::KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.focus.is_focused(window)
            || self.file_page
            || self.canvas.loading
            || self.confirm_discard
        {
            return;
        }
        let key = &event.keystroke.key;
        let mods = event.keystroke.modifiers;
        if self.gesture.is_some() {
            if key == "escape" {
                self.gesture = None;
                self.dragging = false;
                cx.notify();
                cx.stop_propagation();
            }
            return;
        }
        if key == "escape"
            && self
                .canvas
                .reader
                .as_ref()
                .is_some_and(|r| r.engine().painter.is_some())
        {
            let _ = self.try_edit("edit.formatPainter", &serde_json::json!({}), window, cx);
            cx.stop_propagation();
            return;
        }
        // Plain text and active IME keys have exactly one route: the platform input handler.
        if self
            .canvas
            .reader
            .as_ref()
            .is_some_and(|r| r.is_composing())
        {
            if key == "escape" {
                self.apply_input(window, cx, |r| {
                    r.cancel_composition();
                    Ok(())
                });
                cx.stop_propagation();
            }
            return;
        }
        if key == "escape" && self.view_focus() {
            self.run_format(
                "view.focus",
                &serde_json::json!({"value":false}),
                window,
                cx,
            );
            cx.stop_propagation();
            return;
        }
        if mods.control || mods.platform {
            if mods.shift && !mods.alt && (key == "c" || key == "v") {
                let _ = self.try_edit(
                    if key == "c" {
                        "edit.copyFormat"
                    } else {
                        "edit.pasteFormat"
                    },
                    &serde_json::json!({}),
                    window,
                    cx,
                );
                cx.stop_propagation();
                return;
            }
            if key == "d" && !mods.shift && !mods.alt {
                self.format_dialog(formatting::Form::Font, window, cx);
                cx.stop_propagation();
                return;
            }
            let format = match (key.as_str(), mods.shift, mods.alt) {
                ("b", false, false) => Some("format.bold"),
                ("i", false, false) => Some("format.italic"),
                ("u", false, false) => Some("format.underline"),
                ("x", true, false) => Some("format.strikethrough"),
                ("l", false, false) => Some("para.alignLeft"),
                ("e", false, false) => Some("para.alignCenter"),
                ("r", false, false) => Some("para.alignRight"),
                ("j", false, false) => Some("para.justify"),
                ("m", false, false) => Some("para.indent"),
                ("m", true, false) => Some("para.outdent"),
                ("l", true, false) => Some("para.bullets"),
                ("n", true, false) => Some("para.normal"),
                ("1", false, true) => Some("para.heading1"),
                ("2", false, true) => Some("para.heading2"),
                ("1", false, false) => Some("para.single"),
                ("2", false, false) => Some("para.double"),
                ("5", false, false) => Some("para.oneAndHalf"),
                (">", _, false) | (".", true, false) => Some("format.growFont"),
                ("<", _, false) | (",", true, false) => Some("format.shrinkFont"),
                _ => None,
            };
            if let Some(id) = format {
                self.run_format(id, &serde_json::json!({}), window, cx);
                cx.stop_propagation();
                return;
            }
        }
        let command = match key.as_str() {
            "left" => {
                if mods.control || mods.alt {
                    "caret.wordLeft"
                } else {
                    "caret.left"
                }
            }
            "right" => {
                if mods.control || mods.alt {
                    "caret.wordRight"
                } else {
                    "caret.right"
                }
            }
            "up" => "caret.up",
            "down" => "caret.down",
            "home" => {
                if mods.control || mods.platform {
                    "caret.docStart"
                } else {
                    "caret.home"
                }
            }
            "end" => {
                if mods.control || mods.platform {
                    "caret.docEnd"
                } else {
                    "caret.end"
                }
            }
            "backspace" => {
                if mods.control {
                    "text.deleteWordBack"
                } else {
                    "text.backspace"
                }
            }
            "delete" => {
                if mods.control {
                    "text.deleteWordForward"
                } else {
                    "text.delete"
                }
            }
            "enter" => {
                if mods.shift {
                    "text.lineBreak"
                } else {
                    "text.newParagraph"
                }
            }
            "tab" => {
                if mods.shift {
                    "text.backTab"
                } else {
                    "text.tab"
                }
            }
            "escape" => "select.collapse",
            _ => return,
        };
        self.engine_command(
            command,
            &serde_json::json!({"extend":mods.shift}),
            window,
            cx,
        );
        cx.stop_propagation();
    }
    pub(super) fn document_tab(&mut self, reverse: bool, w: &mut Window, cx: &mut Context<Self>) {
        if !self.focus.is_focused(w) {
            cx.propagate();
            return;
        }
        let id = if reverse { "text.backTab" } else { "text.tab" };
        if self.gesture.is_none() && self.format_ready(id) {
            self.engine_command(id, &serde_json::json!({}), w, cx);
        }
        cx.stop_propagation();
    }
    pub(super) fn hit_position(
        &self,
        point: Point<Pixels>,
    ) -> Option<joyword_shell::editing::DocPos> {
        let reader = self.canvas.reader.as_ref()?;
        let scale = self.canvas.factor(self.shell.zoom);
        let (page, bounds) = self
            .page_bounds
            .iter()
            .filter(|(_, b)| point.y >= b.top() && point.y <= b.bottom())
            .min_by_key(|(_, b)| ((f32::from(point.x - b.origin.x)).abs() * 100.0) as u32)?;
        reader.geometry().hit(
            *page,
            f32::from(point.x - bounds.origin.x) / scale,
            f32::from(point.y - bounds.origin.y) / scale,
            joyword_shell::editing::BodyStory,
        )
    }
    pub(super) fn mouse_selection(
        &mut self,
        point: Point<Pixels>,
        extend: bool,
        count: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(pos) = self.hit_position(point) {
            window.focus(&self.focus, cx);
            self.apply_input(window, cx, |r| {
                let anchor = if extend {
                    r.engine().sel.anchor.clone()
                } else {
                    pos.clone()
                };
                r.select_positions(anchor, pos)?;
                if count == 2 {
                    r.edit_command("select.word", &serde_json::json!({}))?;
                } else if count >= 3 {
                    r.edit_command("select.paragraph", &serde_json::json!({}))?;
                }
                Ok(())
            });
        }
    }
    fn caret_bounds(&self, pos: &joyword_shell::editing::DocPos) -> Option<Bounds<Pixels>> {
        let reader = self.canvas.reader.as_ref()?;
        let c = reader.geometry().caret(pos)?;
        let page = self.page_bounds.get(&c.page)?;
        let s = self.canvas.factor(self.shell.zoom);
        Some(Bounds::new(
            point(page.origin.x + px(c.x * s), page.origin.y + px(c.top * s)),
            size(px((c.width * s).max(1.5)), px((c.height * s).max(1.5))),
        ))
    }
    pub(super) fn page_input(
        &self,
        page: usize,
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        let owner = cx.entity();
        div()
            .id(SharedString::from(format!("input-page-{page}")))
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .cursor_text()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |v, event: &gpui::MouseDownEvent, w, cx| {
                    v.pointer_down(page, event, w, cx);
                }),
            )
            .on_mouse_move(cx.listener(|v, event: &gpui::MouseMoveEvent, w, cx| {
                if v.dragging && event.dragging() {
                    v.pointer_move(event, w, cx);
                }
            }))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|v, e, w, cx| v.pointer_up(e, w, cx)),
            )
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(|v, e, w, cx| v.pointer_up(e, w, cx)),
            )
            .child(
                canvas(
                    |bounds, _, _| bounds,
                    move |bounds, _, window, cx| {
                        owner.update(cx, |v, _cx| {
                            v.page_bounds.insert(page, bounds);
                            let caret_page = v.canvas.input_paint.and_then(|_| {
                                let reader = v.canvas.reader.as_ref()?;
                                reader
                                    .geometry()
                                    .caret(&reader.engine().sel.focus)
                                    .map(|c| c.page)
                            });
                            v.canvas.measure_paint(
                                page,
                                caret_page,
                                -f32::from(v.scroll.offset().y),
                                v.shell.zoom,
                            );
                            if let Some(reader) = &v.canvas.reader {
                                let scale = v.canvas.factor(v.shell.zoom);
                                let sel = &reader.engine().sel;
                                let convert = |r: joyword_shell::editing::PageRect| {
                                    Bounds::new(
                                        point(
                                            bounds.origin.x + px(r.x * scale),
                                            bounds.origin.y + px(r.y * scale),
                                        ),
                                        size(px(r.w * scale), px(r.h * scale)),
                                    )
                                };
                                if reader.engine().cell_range().is_none() {
                                    for (p, r) in
                                        joyword_shell::authoring::table_mouse::selection_rects(
                                            reader.geometry(),
                                            &reader.engine().doc,
                                            &sel.anchor,
                                            &sel.focus,
                                            page,
                                        )
                                    {
                                        if p == page {
                                            window.paint_quad(fill(convert(r), rgba(0x4285f440)));
                                        }
                                    }
                                }
                                if reader.is_composing()
                                    && let Some(range) = reader.marked_range()
                                    && let Ok((a, b)) = reader.native_positions(range)
                                {
                                    for (p, r) in reader.geometry().selection_rects(
                                        &reader.engine().doc,
                                        &a,
                                        &b,
                                        page,
                                    ) {
                                        if p == page {
                                            let mut rect = convert(r);
                                            rect.origin.y += rect.size.height - px(1.5);
                                            rect.size.height = px(1.5);
                                            window.paint_quad(fill(rect, rgb(0x1558c0)));
                                        }
                                    }
                                }
                                if v.focus.is_focused(window)
                                    && v.caret_on
                                    && !v.confirm_discard
                                    && let Some(c) = reader.geometry().caret(&sel.focus)
                                    && c.page == page
                                {
                                    let rect = Bounds::new(
                                        point(
                                            bounds.origin.x + px(c.x * scale),
                                            bounds.origin.y + px(c.top * scale),
                                        ),
                                        size(
                                            px((c.width * scale).max(1.5)),
                                            px((c.height * scale).max(1.5)),
                                        ),
                                    );
                                    window.paint_quad(fill(rect, rgb(0x111111)));
                                }
                            }
                            v.paint_authoring(page, bounds, window);
                        });
                    },
                )
                .size_full(),
            )
    }
    pub(super) fn input_registration(&self, cx: &mut Context<Self>) -> gpui::Div {
        let owner = cx.entity();
        let input = self.input.clone();
        let focus = self.focus.clone();
        div().absolute().top_0().left_0().size_full().child(
            canvas(
                move |bounds, _, cx| {
                    owner.update(cx, |v, cx| {
                        let resized = v.viewport_bounds.is_none_or(|old| {
                            (f32::from(old.size.width - bounds.size.width)).abs() >= 1.0
                                || (f32::from(old.size.height - bounds.size.height)).abs() >= 1.0
                        });
                        v.viewport_bounds = Some(bounds);
                        let height = f32::from(bounds.size.height).max(1.0);
                        if resized {
                            v.canvas.viewport = height;
                            cx.notify();
                        }
                        if v.reveal_input {
                            v.reveal_input = false;
                            if let Some(r) = &v.canvas.reader
                                && let Some(c) = r.geometry().caret(&r.engine().sel.focus)
                            {
                                let scale = v.canvas.factor(v.shell.zoom);
                                let top = v.canvas.top(c.page, v.shell.zoom) + 18.0 + c.top * scale;
                                let bottom = top + c.height * scale;
                                let old = v.scroll.offset();
                                let y = -f32::from(old.y);
                                let height = f32::from(bounds.size.height);
                                let next = if top < y {
                                    top - 16.0
                                } else if bottom > y + height {
                                    bottom - height + 16.0
                                } else {
                                    y
                                };
                                v.scroll.set_offset(point(old.x, px(-next.max(0.0))));
                                if old != v.scroll.offset() {
                                    cx.notify();
                                }
                            }
                        }
                    });
                },
                move |bounds, (), window, cx| {
                    if let Some(input) = &input {
                        window.handle_input(
                            &focus,
                            ElementInputHandler::new(bounds, input.clone()),
                            cx,
                        );
                    }
                },
            )
            .size_full(),
        )
    }
}

#[cfg(test)]
mod integration_tests {
    use super::*;
    use gpui::TestAppContext;
    use std::sync::atomic::AtomicBool;
    #[gpui::test]
    fn kit_root_tab_navigates_cells_but_form_tab_keeps_native_focus(cx: &mut TestAppContext) {
        cx.update(gpui_kit::init);
        cx.update(bind_document_keys);
        let mut editor = None;
        let (_, cx) = cx.add_window_view(|window, cx| {
            let view = cx.new(|cx| JoyWord::new(window, cx));
            editor = Some(view.clone());
            gpui::base::Root::new(view, window, cx)
        });
        let view = editor.unwrap();
        view.update(cx, |v, cx| {
            let mut r = joyword_shell::reader::Reader::blank(&AtomicBool::new(false)).unwrap();
            r.execute("insert.table", &serde_json::json!({"rows":2,"cols":2}))
                .unwrap();
            let doc = r.engine().document_id();
            v.canvas.reader = Some(r);
            v.install_input(doc, cx);
        });
        cx.update(|w, cx| w.draw(cx).clear(cx));
        cx.simulate_keystrokes("tab");
        view.read_with(cx, |v, _| {
            assert_eq!(
                v.canvas.reader.as_ref().unwrap().table_info().unwrap().col,
                1
            )
        });
        cx.simulate_keystrokes("shift-tab");
        view.read_with(cx, |v, _| {
            assert_eq!(
                v.canvas.reader.as_ref().unwrap().table_info().unwrap().col,
                0
            )
        });
        // A Kit dialog owns Tab while its input has focus, without editing the document.
        cx.update(|w, cx| view.update(cx, |v, cx| v.insert_table_form(w, cx)));
        cx.update(|w, cx| w.draw(cx).clear(cx));
        let before = view.read_with(cx, |v, _| v.canvas.reader.as_ref().unwrap().engine().rev());
        cx.simulate_keystrokes("tab");
        view.read_with(cx, |v, _| {
            assert_eq!(v.canvas.reader.as_ref().unwrap().engine().rev(), before)
        });
    }
    #[gpui::test]
    fn platform_input_bridge_handles_composition_and_rejects_old_document(cx: &mut TestAppContext) {
        cx.update(gpui_kit::init);
        let (view, cx) = cx.add_window_view(JoyWord::new);
        view.update(cx, |v, cx| {
            let reader = joyword_shell::reader::Reader::blank(&AtomicBool::new(false)).unwrap();
            let id = reader.engine().document_id();
            v.canvas.reader = Some(reader);
            v.install_input(id, cx);
        });
        let input = view.read_with(cx, |v, _| v.input.clone().unwrap());
        cx.update(|window, cx| {
            input.update(cx, |input, cx| {
                input.replace_and_mark_text_in_range(None, "zhong", None, window, cx)
            })
        });
        assert!(view.read_with(cx, |v, _| v.canvas.reader.as_ref().unwrap().is_composing()));
        cx.update(|window, cx| {
            input.update(cx, |input, cx| {
                input.replace_and_mark_text_in_range(None, "zhongwen", None, window, cx)
            })
        });
        cx.update(|window, cx| {
            input.update(cx, |input, cx| {
                input.replace_text_in_range(None, "中文", window, cx)
            })
        });
        view.read_with(cx, |v, _| {
            let r = v.canvas.reader.as_ref().unwrap();
            assert_eq!(r.body_text(), "中文");
            assert!(!r.is_composing());
            assert_eq!(r.engine().undo_depth(), 1);
        });
        view.update(cx, |v, cx| {
            let r = joyword_shell::reader::Reader::blank(&AtomicBool::new(false)).unwrap();
            let id = r.engine().document_id();
            v.canvas.reader = Some(r);
            v.install_input(id, cx);
        });
        cx.update(|window, cx| {
            input.update(cx, |input, cx| {
                input.replace_text_in_range(None, "stale", window, cx)
            })
        });
        assert!(view.read_with(cx, |v, _| {
            v.canvas.reader.as_ref().unwrap().body_text().is_empty()
        }));
        let current = view.read_with(cx, |v, _| v.input.clone().unwrap());
        cx.update(|window, cx| {
            current.update(cx, |input, cx| {
                input.replace_text_in_range(None, "新文档 😀", window, cx)
            })
        });
        assert_eq!(
            view.read_with(cx, |v, _| v.canvas.reader.as_ref().unwrap().body_text()),
            "新文档 😀"
        );
        cx.update(|window, cx| window.draw(cx).clear(cx));
        view.read_with(cx, |v, _| {
            let page = v.page_bounds.get(&0).expect("painted page");
            let viewport = v.viewport_bounds.expect("input viewport");
            assert!(page.top() >= viewport.top() && page.top() < viewport.bottom());
            let reader = v.canvas.reader.as_ref().unwrap();
            let caret = v
                .caret_bounds(
                    &reader
                        .engine()
                        .doc
                        .start_of(joyword_shell::editing::BodyStory),
                )
                .unwrap();
            assert!(
                caret.top() >= page.top() && caret.top() < viewport.bottom(),
                "caret must be on the visible page"
            );
        });
        cx.update(|window, cx| {
            let transient = cx.focus_handle();
            window.focus(&transient, cx);
            view.update(cx, |v, cx| {
                v.confirm_discard = true;
                v.close_pending = true;
                v.execute("file.keep", window, cx);
                assert!(!v.confirm_discard && !v.close_pending);
                assert!(v.focus.is_focused(window));
                v.replace_confirm = Some((0, 0));
                window.focus(&transient, cx);
                v.execute("file.cancel_replace", window, cx);
                assert!(v.replace_confirm.is_none());
                assert!(v.focus.is_focused(window));
            });
        });
    }
}
