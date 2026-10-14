//! GPUI adapter for the platform-neutral input state. Geometry is derived
//! only from the current frame and the live scroll offset, never an old scene.
use crate::*;
use gpui::{
    ElementInputHandler, EntityInputHandler, MouseButton, Pixels, Point, UTF16Selection, canvas,
    fill, point, rgba,
};
use joyword_engine::Selection;
use joyword_foundation::Emu;
use joyword_model::{Bias, Position};
use joyword_shell::input::{Direction, InputError};
use std::ops::Range;

pub const DOC_PAD: f32 = 32.0;
pub const DOC_GAP: f32 = 32.0;

/// The last geometry handed to the platform. Comparing this avoids an
/// endless next-frame loop while refreshing IME coordinates after async layout
/// and scrolling, including scrolls whose offscreen pages are not repainted.
#[derive(Clone, PartialEq)]
pub(crate) struct ImeGeometry {
    key: CanvasRequestKey,
    viewport: Bounds<Pixels>,
    scroll: Point<Pixels>,
    selection: Selection,
}

/// A prompt response applies only to the document version the user saw.
#[derive(Debug, PartialEq, Eq)]
enum CloseDecision {
    Cancel,
    Discard,
    AskAgain,
}
fn close_decision(answer: Option<usize>, expected: u64, current: u64) -> CloseDecision {
    if answer != Some(1) {
        CloseDecision::Cancel
    } else if expected != current {
        CloseDecision::AskAgain
    } else {
        CloseDecision::Discard
    }
}

fn project_page(
    layout: &joyword_layout::LayoutSnapshot,
    viewport: Bounds<Pixels>,
    offset: Point<Pixels>,
    scale: ScreenScale,
    page: usize,
) -> Option<Bounds<Pixels>> {
    let scene = layout.pages().get(page)?;
    let before = layout.pages()[..page]
        .iter()
        .map(|p| scale.logical_pixels(p.height) as f32 + DOC_GAP)
        .sum::<f32>();
    Some(Bounds::new(
        point(
            viewport.origin.x + px(DOC_PAD) + offset.x,
            viewport.origin.y + px(DOC_PAD + before) + offset.y,
        ),
        size(
            px(scale.logical_pixels(scene.width) as f32),
            px(scale.logical_pixels(scene.height) as f32),
        ),
    ))
}
fn local_emu(point: Point<Pixels>, page: Bounds<Pixels>, zoom: u16) -> (Emu, Emu) {
    let emu_per_px = Emu::PER_INCH as f64 / 96.0 / (f64::from(zoom) / 100.0);
    (
        Emu::new((f64::from(f32::from(point.x - page.origin.x)) * emu_per_px).round() as i64),
        Emu::new((f64::from(f32::from(point.y - page.origin.y)) * emu_per_px).round() as i64),
    )
}
fn reveal_offset(
    viewport: Bounds<Pixels>,
    caret: Bounds<Pixels>,
    mut offset: Point<Pixels>,
    content: gpui::Size<Pixels>,
) -> Point<Pixels> {
    let margin = px(8.0);
    if caret.top() < viewport.top() + margin {
        offset.y += viewport.top() + margin - caret.top();
    } else if caret.bottom() > viewport.bottom() - margin {
        offset.y -= caret.bottom() - viewport.bottom() + margin;
    }
    if caret.left() < viewport.left() + margin {
        offset.x += viewport.left() + margin - caret.left();
    } else if caret.right() > viewport.right() - margin {
        offset.x -= caret.right() - viewport.right() + margin;
    }
    offset.x = offset
        .x
        .max(-(content.width - viewport.size.width).max(px(0.0)))
        .min(px(0.0));
    offset.y = offset
        .y
        .max(-(content.height - viewport.size.height).max(px(0.0)))
        .min(px(0.0));
    offset
}

fn input_error_label(error: &InputError) -> &'static str {
    match error {
        InputError::ReadOnly | InputError::Transaction(joyword_engine::TxError::ReadOnly) => {
            "input.read_only"
        }
        InputError::InvalidRange | InputError::InsideSurrogate | InputError::UnknownPosition(_) => {
            "input.invalid_range"
        }
        InputError::InvalidDocument | InputError::UnsupportedBody => "input.unsupported_document",
        InputError::Budget(_) | InputError::GenerationOverflow => "input.limit",
        InputError::StaleComposition => "input.stale",
        InputError::InvalidText(_) => "input.unsupported_text",
        InputError::Undo(_) | InputError::Transaction(_) => "input.rejected",
    }
}

impl JoyWord {
    pub(crate) fn install_input_lifecycle(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let weak = cx.weak_entity();
        window.on_window_should_close(cx, move |window, cx| {
            weak.update(cx, |view, cx| {
                view.cancel_preedit(cx);
                if !view
                    .editor
                    .as_ref()
                    .is_some_and(TextInput::has_unsaved_changes)
                {
                    return true;
                }
                view.request_close(false, window, cx);
                false
            })
            .unwrap_or(false)
        });
        let focus = self.focus_handle.clone();
        self.subscriptions
            .push(cx.on_blur(&focus, window, |view, _, cx| view.cancel_preedit(cx)));
        self.subscriptions
            .push(cx.observe_window_activation(window, |view, window, cx| {
                if !window.is_window_active() {
                    view.cancel_preedit(cx);
                }
            }));
    }
    pub(crate) fn request_close(
        &mut self,
        quit: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.close_pending {
            return;
        }
        self.cancel_preedit(cx);
        let Some(editor) = self.editor.as_ref().filter(|e| e.has_unsaved_changes()) else {
            if quit {
                cx.quit();
            } else {
                window.remove_window();
            }
            return;
        };
        let expected = editor.backend().version().get();
        self.close_pending = true;
        let catalog = Catalog::shipped(self.locale);
        let message = catalog.format("close.unsaved", &[("title", "JoyWord")]);
        let receiver = window.prompt(
            gpui::PromptLevel::Warning,
            &message,
            Some(&self.label("stage")),
            &[
                gpui::PromptButton::cancel(self.label("close.cancel")),
                gpui::PromptButton::new(self.label("close.discard")),
            ],
            cx,
        );
        self.close_task = Some(cx.spawn_in(window, async move |view, cx| {
            let answer = receiver.await.ok();
            let _ = view.update_in(cx, |view, window, cx| {
                view.close_pending = false;
                let Some(editor) = &view.editor else {
                    return;
                };
                match close_decision(answer, expected, editor.backend().version().get()) {
                    CloseDecision::Cancel => {
                        window.focus(&view.focus_handle);
                        cx.notify();
                    }
                    CloseDecision::AskAgain => view.request_close(quit, window, cx),
                    CloseDecision::Discard => {
                        if quit {
                            cx.quit();
                        } else {
                            window.remove_window();
                        }
                    }
                }
            });
        }));
    }
    fn apply_input<R>(
        &mut self,
        cx: &mut Context<Self>,
        f: impl FnOnce(&mut TextInput) -> Result<R, InputError>,
    ) {
        let Some(editor) = &mut self.editor else {
            return;
        };
        let result = f(editor);
        self.input_error = result
            .err()
            .map(|error| self.label(input_error_label(&error)));
        self.preferred_x = None;
        // Committing a preview can publish exactly the same snapshot. Its
        // source IDs, version and content must all match before reusing pixels.
        if let (Some(frame), Some(font), Some(profile), Some(editor)) =
            (&mut self.frame, &self.font, self.profile, &self.editor)
            && frame
                .layout
                .ensure_current(editor.view().snapshot(), font, profile)
                .is_ok()
        {
            frame.key.revision = editor.view().revision();
        }
        self.reveal_pending = true;
        self.reveal_caret();
        cx.notify();
    }
    fn cancel_preedit(&mut self, cx: &mut Context<Self>) {
        self.dragging = false;
        if self.editor.as_ref().is_some_and(TextInput::is_composing) {
            self.apply_input(cx, TextInput::cancel_composition);
        }
    }
    pub(crate) fn input_availability(&self, id: CommandId) -> Availability {
        if !id.0.starts_with("edit.") {
            return Availability::Enabled;
        }
        let Some(editor) = &self.editor else {
            return Availability::Disabled("input.loading");
        };
        match id {
            EDIT_UNDO if !editor.can_undo() && !editor.is_composing() => {
                Availability::Disabled("input.no_undo")
            }
            EDIT_REDO if !editor.can_redo() => Availability::Disabled("input.no_redo"),
            EDIT_REDO if editor.is_composing() => Availability::Disabled("input.composing"),
            EDIT_CANCEL if !editor.is_composing() => Availability::Disabled("input.no_composition"),
            EDIT_UNDO | EDIT_REDO | EDIT_BACKSPACE | EDIT_DELETE | EDIT_ENTER
                if editor.read_only() =>
            {
                Availability::Disabled("input.read_only")
            }
            EDIT_UP | EDIT_DOWN | EDIT_EXTEND_UP | EDIT_EXTEND_DOWN | EDIT_HOME | EDIT_END
            | EDIT_EXTEND_HOME | EDIT_EXTEND_END
                if self.ready_frame().is_none() =>
            {
                Availability::Disabled("input.layout_pending")
            }
            _ => Availability::Enabled,
        }
    }
    pub(crate) fn execute_input(
        &mut self,
        id: CommandId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        match id {
            EDIT_UNDO => self.apply_input(cx, TextInput::undo),
            EDIT_REDO => self.apply_input(cx, TextInput::redo),
            EDIT_SELECT_ALL => self.apply_input(cx, TextInput::select_all),
            EDIT_BACKSPACE => self.apply_input(cx, |e| e.delete(Direction::Backward)),
            EDIT_DELETE => self.apply_input(cx, |e| e.delete(Direction::Forward)),
            EDIT_ENTER => self.apply_input(cx, |e| e.replace(None, "\n")),
            EDIT_CANCEL => self.apply_input(cx, TextInput::cancel_composition),
            EDIT_LEFT | EDIT_RIGHT | EDIT_EXTEND_LEFT | EDIT_EXTEND_RIGHT => {
                let direction = if id == EDIT_LEFT || id == EDIT_EXTEND_LEFT {
                    Direction::Backward
                } else {
                    Direction::Forward
                };
                let extend = id == EDIT_EXTEND_LEFT || id == EDIT_EXTEND_RIGHT;
                self.apply_input(cx, |e| e.move_grapheme(direction, extend));
            }
            EDIT_UP | EDIT_DOWN | EDIT_EXTEND_UP | EDIT_EXTEND_DOWN | EDIT_HOME | EDIT_END
            | EDIT_EXTEND_HOME | EDIT_EXTEND_END => self.move_visual(id, cx),
            _ => return Err(format!("{id} is not an input command")),
        }
        window.invalidate_character_coordinates();
        Ok(())
    }
    fn move_visual(&mut self, id: CommandId, cx: &mut Context<Self>) {
        if self.editor.as_ref().is_some_and(TextInput::is_composing) {
            return;
        }
        let Some(editor) = &self.editor else {
            return;
        };
        let Some(frame) = self.ready_frame() else {
            return;
        };
        let selection = editor.view().selection();
        let Ok(caret) = frame.layout.caret(selection.head) else {
            return;
        };
        let Some(i) = frame
            .layout
            .lines()
            .iter()
            .position(|l| l.page == caret.page && l.top == caret.top)
        else {
            return;
        };
        let lines = frame.layout.lines();
        let vertical = matches!(id, EDIT_UP | EDIT_DOWN | EDIT_EXTEND_UP | EDIT_EXTEND_DOWN);
        let extend = matches!(
            id,
            EDIT_EXTEND_UP | EDIT_EXTEND_DOWN | EDIT_EXTEND_HOME | EDIT_EXTEND_END
        );
        let x = self.preferred_x.unwrap_or(caret.x);
        let target = if vertical {
            let j = if id == EDIT_UP || id == EDIT_EXTEND_UP {
                i.saturating_sub(1)
            } else {
                (i + 1).min(lines.len() - 1)
            };
            let line = &lines[j];
            frame
                .layout
                .hit_test(
                    line.page,
                    x,
                    Emu::new(line.top.raw() + line.height.raw() / 2),
                )
                .ok()
        } else {
            let line = &lines[i];
            let start = id == EDIT_HOME || id == EDIT_EXTEND_HOME;
            let stop = if start {
                line.carets.first()
            } else {
                line.carets.last()
            };
            stop.map(|stop| Position {
                paragraph: line.paragraph,
                offset: stop.offset,
                bias: if start { Bias::After } else { Bias::Before },
                ..selection.head
            })
        };
        if let Some(head) = target {
            self.apply_input(cx, |e| {
                e.select_positions(Selection {
                    anchor: if extend { selection.anchor } else { head },
                    head,
                })
            });
            if vertical {
                self.preferred_x = Some(x);
            }
        }
    }
    pub(crate) fn input_status(&self) -> String {
        if let Some(reason) = &self.input_error {
            return Catalog::shipped(self.locale).format("input.error", &[("reason", reason)]);
        }
        if let Some(frame) = self.ready_frame()
            && !frame.layout.diagnostics().is_empty()
        {
            return self.label("input.missing_glyphs");
        }
        if self.frame.is_some() && self.ready_frame().is_none() {
            return self.label("input.layout_pending");
        }
        if let Some(editor) = &self.editor {
            if editor.is_composing() {
                return self.label("input.composing");
            }
            if editor.has_unsaved_changes() {
                return self.label("input.dirty");
            }
        }
        self.label("status")
    }
    fn scale(&self) -> ScreenScale {
        ScreenScale::new(f64::from(self.zoom_percent) / 100.0).expect("clamped zoom")
    }
    fn page_bounds(&self, page: usize) -> Option<Bounds<Pixels>> {
        project_page(
            &self.ready_frame()?.layout,
            self.viewport_bounds?,
            self.scroll.offset(),
            self.scale(),
            page,
        )
    }
    pub(crate) fn reveal_caret(&mut self) {
        if !self.reveal_pending {
            return;
        }
        let Some(viewport) = self.viewport_bounds else {
            return;
        };
        let Some(editor) = &self.editor else {
            return;
        };
        let Some(frame) = self.ready_frame() else {
            return;
        };
        let Ok(c) = frame.layout.caret(editor.view().selection().head) else {
            return;
        };
        let Some(rect) = self.scene_rect(c.page, c.x, c.top, Emu::new(1), c.height) else {
            return;
        };
        let scale = self.scale();
        let width = frame
            .layout
            .pages()
            .iter()
            .map(|p| scale.logical_pixels(p.width) as f32)
            .fold(0.0f32, f32::max)
            + 2.0 * DOC_PAD;
        let height = frame
            .layout
            .pages()
            .iter()
            .map(|p| scale.logical_pixels(p.height) as f32)
            .sum::<f32>()
            + frame.layout.pages().len().saturating_sub(1) as f32 * DOC_GAP
            + 2.0 * DOC_PAD;
        let offset = reveal_offset(
            viewport,
            rect,
            self.scroll.offset(),
            size(px(width), px(height)),
        );
        self.scroll.set_offset(offset);
        self.reveal_pending = false;
    }
    fn scene_rect(
        &self,
        page: usize,
        x: Emu,
        top: Emu,
        width: Emu,
        height: Emu,
    ) -> Option<Bounds<Pixels>> {
        let bounds = self.page_bounds(page)?;
        let scale = self.scale();
        Some(Bounds::new(
            point(
                bounds.origin.x + px(scale.logical_pixels(x) as f32),
                bounds.origin.y + px(scale.logical_pixels(top) as f32),
            ),
            size(
                px((scale.logical_pixels(width) as f32).max(1.0)),
                px(scale.logical_pixels(height) as f32),
            ),
        ))
    }
    fn hit(&self, point: Point<Pixels>, clamp: bool) -> Option<Position> {
        let frame = self.ready_frame()?;
        let viewport = self.viewport_bounds?;
        if !clamp && !viewport.contains(&point) {
            return None;
        }
        let page = (0..frame.layout.pages().len())
            .filter_map(|i| self.page_bounds(i).map(|b| (i, b)))
            .filter(|(_, b)| clamp || b.contains(&point))
            .min_by(|(_, a), (_, b)| {
                let distance = |r: &Bounds<Pixels>| {
                    (f32::from(point.y - r.origin.y) - f32::from(r.size.height) / 2.0).abs()
                };
                distance(a).total_cmp(&distance(b))
            })?
            .0;
        let bounds = self.page_bounds(page)?;
        let (x, y) = local_emu(point, bounds, self.zoom_percent);
        frame.layout.hit_test(page, x, y).ok()
    }
    fn mouse_select(
        &mut self,
        point: Point<Pixels>,
        extend: bool,
        clicks: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // Reject stale geometry before changing either composition or selection.
        if self.hit(point, extend).is_none() {
            return;
        }
        if self.editor.as_ref().is_some_and(TextInput::is_composing) {
            self.apply_input(cx, TextInput::unmark);
        }
        let Some(head) = self.hit(point, extend) else {
            return;
        };
        let Some(editor) = &self.editor else {
            return;
        };
        let anchor = if extend {
            editor.view().selection().anchor
        } else {
            head
        };
        let offset = editor.view().index().offset(head).ok();
        self.apply_input(cx, |editor| {
            if clicks >= 3 {
                editor.select_paragraph_at(offset.ok_or(InputError::InvalidRange)?)
            } else if clicks == 2 {
                editor.select_word_at(offset.ok_or(InputError::InvalidRange)?)
            } else {
                editor.select_positions(Selection { anchor, head })
            }
        });
        window.focus(&self.focus_handle);
        window.invalidate_character_coordinates();
    }
    fn refresh_ime_geometry(&mut self, window: &mut Window) {
        let geometry = self.ready_frame().and_then(|frame| {
            Some(ImeGeometry {
                key: frame.key.clone(),
                viewport: self.viewport_bounds?,
                scroll: self.scroll.offset(),
                selection: self.editor.as_ref()?.view().selection(),
            })
        });
        if geometry != self.ime_geometry {
            self.ime_geometry = geometry;
            if self.ime_geometry.is_some() {
                window.invalidate_character_coordinates();
            }
        }
    }
    fn paint_input(&self, bounds: Bounds<Pixels>, window: &mut Window) {
        let Some(editor) = &self.editor else {
            return;
        };
        let Some(frame) = self.ready_frame() else {
            return;
        };
        let selection = editor.view().selection();
        let (range, _) = editor.view().selected_range();
        window.with_content_mask(Some(gpui::ContentMask { bounds }), |window| {
            let index = editor.view().index();
            let rectangles = |range: Range<usize>| {
                let start = index.position(range.start, Bias::After).ok()?;
                let end = index.position(range.end, Bias::Before).ok()?;
                frame.layout.selection_rects(start, end).ok()
            };
            if let Some(rects) = rectangles(range.clone()) {
                for r in rects {
                    if let Some(b) = self.scene_rect(r.page, r.x, r.top, r.width, r.height) {
                        window.paint_quad(fill(b, rgba(0x3984d640)));
                    }
                }
            }
            if let Some(marked) = editor.view().marked_range()
                && let Some(rects) = rectangles(marked)
            {
                for r in rects {
                    if let Some(mut b) = self.scene_rect(r.page, r.x, r.top, r.width, r.height) {
                        b.origin.y += b.size.height - px(1.5);
                        b.size.height = px(1.5);
                        window.paint_quad(fill(b, rgb(0x1f5b9b)));
                    }
                }
            }
            if self.focus_handle.is_focused(window)
                && window.is_window_active()
                && let Ok(c) = frame.layout.caret(selection.head)
                && let Some(mut b) = self.scene_rect(c.page, c.x, c.top, Emu::new(1), c.height)
            {
                b.size.width = px(1.5);
                window.paint_quad(fill(b, rgb(0x163b65)));
            }
        });
    }
    pub(crate) fn input_viewport(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let entity = cx.entity();
        let prepaint_entity = entity.clone();
        div()
            .id("input-viewport")
            .relative()
            .flex_1()
            .min_h_0()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|view, event: &gpui::MouseDownEvent, window, cx| {
                    view.dragging = view.hit(event.position, false).is_some();
                    view.mouse_select(
                        event.position,
                        event.modifiers.shift,
                        event.click_count,
                        window,
                        cx,
                    );
                }),
            )
            .on_mouse_move(
                cx.listener(|view, event: &gpui::MouseMoveEvent, window, cx| {
                    if view.dragging && event.dragging() {
                        view.mouse_select(event.position, true, 1, window, cx);
                    }
                }),
            )
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|view, _, _, _| view.dragging = false),
            )
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(|view, _, _, _| view.dragging = false),
            )
            .child(
                div()
                    .id("page-scroll")
                    .size_full()
                    .overflow_scroll()
                    .track_scroll(&self.scroll)
                    .p(px(DOC_PAD))
                    .child(self.canvas()),
            )
            .child(
                canvas(
                    move |bounds, _, cx| {
                        prepaint_entity.update(cx, |view, _| {
                            view.viewport_bounds = Some(bounds);
                        });
                    },
                    move |bounds, (), window, cx| {
                        let focus = entity.read(cx).focus_handle.clone();
                        window.handle_input(
                            &focus,
                            ElementInputHandler::new(bounds, entity.clone()),
                            cx,
                        );
                        entity.update(cx, |view, _| {
                            view.refresh_ime_geometry(window);
                            view.paint_input(bounds, window);
                        });
                    },
                )
                .absolute()
                .top_0()
                .left_0()
                .size_full(),
            )
            .into_any_element()
    }
}

impl EntityInputHandler for JoyWord {
    fn text_for_range(
        &mut self,
        range: Range<usize>,
        adjusted: &mut Option<Range<usize>>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<String> {
        *adjusted = None;
        let text = self
            .editor
            .as_ref()?
            .view()
            .index()
            .text_for_range(range.clone())
            .ok()?;
        *adjusted = Some(range);
        Some(text)
    }
    fn selected_text_range(
        &mut self,
        ignore_disabled: bool,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        let editor = self.editor.as_ref()?;
        if editor.read_only() && !ignore_disabled {
            return None;
        }
        let (range, reversed) = editor.view().selected_range();
        Some(UTF16Selection { range, reversed })
    }
    fn marked_text_range(&self, _: &mut Window, _: &mut Context<Self>) -> Option<Range<usize>> {
        self.editor.as_ref()?.view().marked_range()
    }
    fn unmark_text(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.apply_input(cx, TextInput::unmark);
        window.invalidate_character_coordinates();
    }
    fn replace_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.apply_input(cx, |e| e.replace(range, text));
        window.invalidate_character_coordinates();
    }
    fn replace_and_mark_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        selected: Option<Range<usize>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.apply_input(cx, |e| e.mark(range, text, selected));
        window.invalidate_character_coordinates();
    }
    fn bounds_for_range(
        &mut self,
        range: Range<usize>,
        _: Bounds<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let index = self.editor.as_ref()?.view().index();
        index.text_for_range(range.clone()).ok()?;
        let frame = self.ready_frame()?;
        let head = self.editor.as_ref()?.view().selection().head;
        let start = if range.is_empty() && index.offset(head).ok() == Some(range.start) {
            head
        } else {
            index.position(range.start, Bias::After).ok()?
        };
        let end = index.position(range.end, Bias::Before).ok()?;
        if range.is_empty() {
            let c = frame.layout.caret(start).ok()?;
            self.scene_rect(c.page, c.x, c.top, Emu::new(1), c.height)
        } else {
            let r = frame
                .layout
                .selection_rects(start, end)
                .ok()?
                .into_iter()
                .next()?;
            self.scene_rect(r.page, r.x, r.top, r.width, r.height)
        }
    }
    fn character_index_for_point(
        &mut self,
        point: Point<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<usize> {
        let position = self.hit(point, false)?;
        self.editor.as_ref()?.view().index().offset(position).ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn page_geometry_round_trips_at_zoom_with_live_scroll_on_multiple_pages() {
        let font = LoadedFont::from_bytes(
            Arc::from(
                include_bytes!("../../../tests/fixtures/fonts/joyoffice-test-serif.ttf").as_slice(),
            ),
            0,
        )
        .unwrap();
        let source =
            document_canvas::source_with_text(font, &vec!["AV fi"; 100].join("\n")).unwrap();
        assert!(source.layout.pages().len() > 1);
        let viewport = Bounds::new(point(px(17.0), px(60.0)), size(px(800.0), px(500.0)));
        for zoom in [50, 80, 100, 150] {
            let scale = ScreenScale::new(f64::from(zoom) / 100.0).unwrap();
            for offset in [point(px(0.0), px(0.0)), point(px(-50.0), px(-1400.0))] {
                for line in source.layout.lines() {
                    let page =
                        project_page(&source.layout, viewport, offset, scale, line.page).unwrap();
                    let stop = line.carets.first().unwrap();
                    let physical = point(
                        page.origin.x + px(scale.logical_pixels(stop.x) as f32),
                        page.origin.y + px(scale.logical_pixels(line.top) as f32),
                    );
                    let (x, y) = local_emu(physical, page, zoom);
                    // Logical pixel f32 precision is bounded before integer EMU conversion.
                    assert!((x.raw() - stop.x.raw()).abs() <= 4);
                    assert!((y.raw() - line.top.raw()).abs() <= 4);
                    let hit = source
                        .layout
                        .hit_test(line.page, x, Emu::new(y.raw() + line.height.raw() / 2))
                        .unwrap();
                    assert_eq!(hit.paragraph, line.paragraph);
                    assert_eq!(hit.offset, stop.offset);
                }
            }
        }
    }
    #[test]
    fn caret_reveal_scrolls_minimally_and_clamps_at_content_edges() {
        let viewport = Bounds::new(point(px(20.0), px(60.0)), size(px(500.0), px(400.0)));
        let content = size(px(1000.0), px(2000.0));
        let offset = point(px(-40.0), px(-200.0));
        let inside = Bounds::new(point(px(50.0), px(100.0)), size(px(1.0), px(24.0)));
        assert_eq!(reveal_offset(viewport, inside, offset, content), offset);
        let below = Bounds::new(point(px(550.0), px(470.0)), size(px(1.0), px(24.0)));
        assert_eq!(
            reveal_offset(viewport, below, offset, content),
            point(px(-79.0), px(-242.0))
        );
        let above = Bounds::new(point(px(-1000.0), px(-1000.0)), size(px(1.0), px(24.0)));
        assert_eq!(
            reveal_offset(viewport, above, offset, content),
            point(px(0.0), px(0.0))
        );
        let far = Bounds::new(point(px(10000.0), px(10000.0)), size(px(1.0), px(24.0)));
        assert_eq!(
            reveal_offset(viewport, far, offset, content),
            point(px(-500.0), px(-1600.0))
        );
    }
    #[test]
    fn only_explicit_discard_for_the_prompted_version_can_close() {
        let v = 4;
        assert_eq!(close_decision(None, v, v), CloseDecision::Cancel);
        for answer in [0, 2, usize::MAX] {
            assert_eq!(close_decision(Some(answer), v, v), CloseDecision::Cancel);
        }
        assert_eq!(close_decision(Some(1), v, v), CloseDecision::Discard);
        assert_eq!(close_decision(Some(1), v, 5), CloseDecision::AskAgain);
    }
}
