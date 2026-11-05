//! Native input/clipboard adapter adapted from JoyOffice apps/joydeck/src/canvas_text.rs.
//! Layout, caret, hit and selection use the shared DeckCraft renderer, not a second shaper.
use super::*;
use joydeck_engine::{
    Command, TableAction, TextTarget,
    text::{self, Draft},
};
use joydeck_model::{ShapeKind, edit as ed, resolve, text::RunKind};
use std::ops::Range;
pub(super) struct Inline {
    pub(super) target: TextTarget,
    pub(super) draft: Draft,
    placed: Option<joydeck_render::PlacedText>,
    affinity: joydeck_text::Affinity,
    anchor: Option<usize>,
}
impl Inline {
    fn reflow(&mut self, model: &joydeck_model::Presentation, page: usize) {
        let (id, cell) = match self.target {
            TextTarget::Shape { shape } => (shape, None),
            TextTarget::Cell { shape, row, col } => (shape, Some((row, col))),
            TextTarget::Notes => return,
        };
        self.placed = joydeck_render::editable_text(model, page, id, cell);
    }
}
impl Viewer {
    pub(super) fn edit_notes(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        if self.composing() {
            return;
        }
        self.end_inline(cx);
        self.notes_visible = true;
        self.sync_notes(w, cx);
        self.notes_input.update(cx, |s, cx| s.focus(w, cx));
        cx.notify();
    }
    pub(super) fn composing(&self) -> bool {
        self.inline
            .as_ref()
            .is_some_and(|i| i.draft.marked.is_some())
    }
    pub(super) fn end_inline(&mut self, cx: &mut Context<Self>) {
        if self.inline.take().is_some() {
            if let Some(e) = &self.editor {
                match e.display(cx) {
                    Ok(d) => self.deck = Some(Arc::new(d)),
                    Err(e) => self.message = e,
                }
            }
            self.render_generation += 1;
            self.image = None;
            self.pending = None;
            cx.notify();
        }
    }
    pub(super) fn cancel_inline(&mut self, cx: &mut Context<Self>) {
        if self.composing() {
            if let Some(i) = &mut self.inline {
                i.draft.cancel();
            }
            self.preview_inline(cx);
        } else {
            self.end_inline(cx);
        }
    }
    pub(super) fn begin_inline(&mut self, _: &ClickEvent, w: &mut Window, cx: &mut Context<Self>) {
        self.begin_selected_text(w, cx);
    }
    pub(super) fn begin_selected_text(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        if self.loading {
            return;
        }
        self.end_inline(cx);
        let result = (|| {
            let e = self.editor.as_ref().ok_or("请先打开文稿")?;
            let id = *e
                .session
                .selection()
                .shapes
                .first()
                .ok_or("请先选中文字对象或表格")?;
            let s = e.session.model().slides[self.page]
                .shape(id)
                .ok_or("对象不存在")?;
            let ctx =
                resolve::Ctx::for_slide(e.session.model(), &e.session.model().slides[self.page])
                    .ok_or("缺少版式")?;
            let x = resolve::xfrm(&ctx, s);
            let bp = resolve::body(&ctx, s);
            if x.rot != 0.
                || bp.rot.is_some_and(|r| r != 0.)
                || bp.warp.is_some()
                || bp
                    .vert
                    .is_some_and(|v| v != joydeck_model::text::TextDir::Horizontal)
            {
                return Err("本切片原位输入支持水平未旋转文字；此对象仍可保留、浏览和另存".into());
            }
            let target = if matches!(s.kind, ShapeKind::Table(_)) {
                let (_, r, c) = self
                    .cell
                    .filter(|(shape, _, _)| *shape == id)
                    .unwrap_or((id, 0, 0));
                TextTarget::Cell {
                    shape: id,
                    row: r,
                    col: c,
                }
            } else {
                TextTarget::Shape { shape: id }
            };
            let mut body = joydeck_engine::text_body(e.session.model(), self.page, &target)?;
            if body.paragraphs.is_empty() {
                body.paragraphs.push(joydeck_model::Paragraph::default());
            }
            text::validate(&body)?;
            if body.body.rot.is_some_and(|r| r != 0.)
                || body
                    .body
                    .vert
                    .is_some_and(|v| v != joydeck_model::text::TextDir::Horizontal)
                || (matches!(&target,TextTarget::Cell{row,col,..} if matches!(&s.kind,ShapeKind::Table(t) if t.cell(*row,*col).is_some_and(|c|c.vertical.is_some_and(|v|v != joydeck_model::text::TextDir::Horizontal)))))
            {
                return Err("竖排/旋转单元格文字暂不支持原位输入；源内容保留".into());
            }
            if body
                .paragraphs
                .iter()
                .flat_map(|p| &p.runs)
                .any(|r| matches!(r.kind, RunKind::Field { .. } | RunKind::Math { .. }))
            {
                return Err("动态字段/公式保持原样；请先转换后再编辑".into());
            }
            Ok((target, body))
        })();
        match result {
            Ok((target, body)) => {
                self.cell = match &target {
                    TextTarget::Cell { shape, row, col } => Some((*shape, *row, *col)),
                    _ => None,
                };
                self.inline = Some(Box::new(Inline {
                    target,
                    draft: Draft::new(body),
                    placed: None,
                    affinity: joydeck_text::Affinity::Downstream,
                    anchor: None,
                }));
                self.drag = None;
                self.preview_inline(cx);
                self.message = "原位输入 · Esc 取消组合/退出 · Ctrl+S 保存 · Tab 下一单元格".into();
                self.focus.focus(w, cx);
            }
            Err(e) => self.message = e,
        }
        cx.notify();
    }
    fn preview_inline(&mut self, cx: &mut Context<Self>) {
        let Some(i) = &mut self.inline else { return };
        let Some(e) = &self.editor else { return };
        let mut model = e.session.model().clone();
        if let Err(e) =
            joydeck_engine::set_draft_body(&mut model, self.page, &i.target, i.draft.body.clone())
        {
            self.message = e;
            return;
        }
        i.reflow(&model, self.page);
        match Deck::from_model(
            self.deck
                .as_ref()
                .map(|d| d.info.name.clone())
                .unwrap_or_default(),
            model,
        ) {
            Ok(d) => self.deck = Some(Arc::new(d)),
            Err(e) => self.message = e,
        }
        self.paint_cancel.store(true, Ordering::Relaxed);
        self.render_generation += 1;
        self.pending = None;
        self.thumbs_cancel.store(true, Ordering::Relaxed);
        self.thumbs_cancel = Arc::new(AtomicBool::new(false));
        self.thumb_busy = false;
        self.thumbs_task = None;
        cx.notify();
    }

    pub(super) fn refresh_inline_layout(&mut self) {
        let (Some(i), Some(deck)) = (&mut self.inline, &self.deck) else {
            return;
        };
        i.reflow(&deck.presentation, self.page);
    }
    pub(super) fn publish_inline(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        if self.composing() {
            self.preview_inline(cx);
            return;
        }
        let Some(i) = self.inline.take() else { return };
        let previous_image = self.image.clone();
        self.dispatch(
            Command::SetText {
                target: i.target.clone(),
                body: Box::new(i.draft.body.clone()),
            },
            w,
            cx,
        );
        self.image = previous_image;
        self.inline = Some(i);
        self.preview_inline(cx);
    }
    fn input_replace(
        &mut self,
        r: Option<Range<usize>>,
        value: &str,
        mark: bool,
        selection: Option<Range<usize>>,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(i) = &mut self.inline else { return };
        match i.draft.replace(r, value, mark, selection) {
            Ok(()) => {
                eprintln!(
                    "R3 input={} utf16={} marked={}",
                    if mark { "preedit" } else { "commit" },
                    value.encode_utf16().count(),
                    self.composing()
                );
                if mark {
                    self.preview_inline(cx)
                } else {
                    self.publish_inline(w, cx)
                }
            }
            Err(e) => {
                self.message = e;
                cx.notify();
            }
        }
        w.notify_text_input_changed(&self.focus);
    }
    fn paste_text(&mut self, item: ClipboardItem, w: &mut Window, cx: &mut Context<Self>) {
        let rich = item
            .metadata()
            .filter(|m| m.len() <= 1024 * 1024)
            .and_then(|m| serde_json::from_str::<serde_json::Value>(m).ok())
            .filter(|m| m["joydeck-text"] == 1)
            .and_then(|m| {
                serde_json::from_value::<joydeck_model::TextBody>(m["body"].clone()).ok()
            });
        if let Some(body) = rich {
            let result = (|| {
                text::validate(&body)?;
                let i = self.inline.as_mut().ok_or("not editing text")?;
                let value = i.draft.body.text();
                if value.len().saturating_sub(i.draft.selection.len()) + body.text().len() > 65536 {
                    return Err("paste exceeds 64 KiB".into());
                }
                let mut draft = i.draft.body.clone();
                let a = text::pos(&value, i.draft.selection.start);
                let b = text::pos(&value, i.draft.selection.end);
                ed::delete(&mut draft, a, b);
                let caret = ed::insert_body(&mut draft, a, &body);
                text::validate(&draft)?;
                let at = text::byte_pos(&draft.text(), caret);
                i.draft.body = draft;
                i.draft.selection = at..at;
                i.draft.reversed = false;
                Ok::<_, String>(())
            })();
            match result {
                Ok(()) => self.publish_inline(w, cx),
                Err(e) => {
                    self.message = e;
                    cx.notify();
                }
            }
        } else if let Some(value) = item.text() {
            self.input_replace(None, &value, false, None, w, cx);
        }
    }
    fn inline_hit(&self, p: Point<Pixels>, scale: f32) -> Option<usize> {
        let i = self.inline.as_ref()?;
        let placed = i.placed.as_ref()?;
        let p = joydeck_geom::Point::new(
            f32::from(p.x - self.canvas_bounds.left()) as f64 / scale as f64,
            f32::from(p.y - self.canvas_bounds.top()) as f64 / scale as f64,
        );
        let pos = placed.layout.hit(placed.transform.inverse() * p);
        Some(text::byte_pos(&i.draft.body.text(), (pos.para, pos.ch)))
    }
    pub(super) fn inline_pointer(
        &mut self,
        event: &MouseDownEvent,
        scale: f32,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.inline.is_none() {
            return false;
        }
        if self.composing() {
            return true;
        }
        let in_box = self
            .inline
            .as_ref()
            .and_then(|i| i.placed.as_ref())
            .is_some_and(|p| {
                let q = joydeck_geom::Point::new(
                    f32::from(event.position.x - self.canvas_bounds.left()) as f64 / scale as f64,
                    f32::from(event.position.y - self.canvas_bounds.top()) as f64 / scale as f64,
                );
                p.layout.inner.contains(p.transform.inverse() * q)
            });
        if !in_box {
            self.end_inline(cx);
            return false;
        }
        if let Some(at) = self.inline_hit(event.position, scale)
            && let Some(i) = &mut self.inline
        {
            i.draft.move_to(at, event.modifiers.shift);
            i.anchor = Some(i.draft.selection.start);
        }
        self.focus.focus(w, cx);
        w.notify_text_selection_changed(&self.focus);
        cx.notify();
        true
    }
    pub(super) fn inline_move(
        &mut self,
        event: &MouseMoveEvent,
        scale: f32,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.inline.is_none() {
            return false;
        }
        if event.dragging()
            && !self.composing()
            && let Some(at) = self.inline_hit(event.position, scale)
            && let Some(i) = &mut self.inline
            && let Some(anchor) = i.anchor
        {
            i.draft.selection = anchor.min(at)..anchor.max(at);
            i.draft.reversed = at < anchor;
            cx.notify();
        }
        true
    }
    fn screen_rect(
        &self,
        p: &joydeck_render::PlacedText,
        rect: joydeck_geom::Rect,
        scale: f32,
    ) -> Bounds<Pixels> {
        let b = p.transform.transform_rect_bbox(rect);
        Bounds::new(
            point(
                self.canvas_bounds.left() + px(b.x0 as f32 * scale),
                self.canvas_bounds.top() + px(b.y0 as f32 * scale),
            ),
            size(
                px((b.width() as f32 * scale).max(1.5)),
                px((b.height() as f32 * scale).max(1.5)),
            ),
        )
    }
    pub(super) fn paint_inline(
        &mut self,
        bounds: Bounds<Pixels>,
        scale: f32,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.canvas_bounds = bounds;
        let Some(i) = &self.inline else { return };
        let Some(p) = &i.placed else { return };
        let value = i.draft.body.text();
        let a = text::pos(&value, i.draft.selection.start);
        let b = text::pos(&value, i.draft.selection.end);
        let a = joydeck_text::Pos { para: a.0, ch: a.1 };
        let b = joydeck_text::Pos { para: b.0, ch: b.1 };
        w.with_content_mask(Some(ContentMask { bounds }), |w| {
            for rect in p.layout.selection_rects(a, b) {
                w.paint_quad(fill(self.screen_rect(p, rect, scale), rgba(0x2299ff55)));
            }
            let at = text::pos(&value, i.draft.cursor());
            if let Some((x, top, bottom)) = p.layout.caret_at(
                joydeck_text::Pos {
                    para: at.0,
                    ch: at.1,
                },
                i.affinity,
            ) && self.focus.is_focused(w)
            {
                w.paint_quad(fill(
                    self.screen_rect(p, joydeck_geom::Rect::new(x, top, x + 1., bottom), scale),
                    rgb(0x176fca),
                ));
            }
            if let Some(marked) = &i.draft.marked {
                let a = text::pos(&value, marked.start);
                let b = text::pos(&value, marked.end);
                for (x0, x1, y) in p.layout.composition_underlines(
                    joydeck_text::Pos { para: a.0, ch: a.1 },
                    joydeck_text::Pos { para: b.0, ch: b.1 },
                ) {
                    w.paint_quad(fill(
                        self.screen_rect(p, joydeck_geom::Rect::new(x0, y, x1, y + 1.), scale),
                        rgb(0x176fca),
                    ));
                }
            }
        });
        if self.focus.is_focused(w) {
            w.handle_input(
                &self.focus,
                ElementInputHandler::new(bounds, cx.entity()),
                cx,
            );
        }
    }
    pub(super) fn inline_key(
        &mut self,
        event: &KeyDownEvent,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.focus.is_focused(w) {
            return;
        }
        let key = event.keystroke.key.as_str();
        let mods = event.keystroke.modifiers;
        let command = mods.control || mods.platform;
        // GPUI omits disabled native actions from key dispatch. Report the same reason
        // when a shortcut falls through to the canvas instead of the native action.
        if command
            && !mods.alt
            && key == "s"
            && let Some(reason) = self.save_unavailable()
        {
            self.message = reason.into();
            cx.notify();
            cx.stop_propagation();
            return;
        }
        if let Some(larger) = format_ui::font_size_direction(command, mods.shift, key)
            && !self.composing()
        {
            self.step_font_size(larger, w, cx);
            cx.stop_propagation();
            return;
        }
        if self.inline.is_none() {
            let basic = match (command, mods.shift, key) {
                (true, false, "z") => Some(BasicEdit::Undo),
                (true, true, "z") => Some(BasicEdit::Redo),
                (true, false, "y") if mods.control => Some(BasicEdit::Redo),
                (true, false, "d") => Some(BasicEdit::Duplicate),
                (true, false, "m") => Some(BasicEdit::NewSlide),
                (false, false, "delete") => Some(BasicEdit::Delete),
                _ => None,
            };
            if !mods.alt
                && let Some(edit) = basic
            {
                self.run_basic_edit(edit, w, cx);
                cx.stop_propagation();
                return;
            }
            let selected = self
                .editor
                .as_ref()
                .is_some_and(|e| !e.session.selection().shapes.is_empty());
            if selected && matches!(key, "left" | "right" | "up" | "down") {
                let step = if mods.shift { 10. } else { 1. };
                self.dispatch(
                    Command::Transform {
                        dx: if key == "left" {
                            -step
                        } else if key == "right" {
                            step
                        } else {
                            0.
                        },
                        dy: if key == "up" {
                            -step
                        } else if key == "down" {
                            step
                        } else {
                            0.
                        },
                        dw: 0.,
                        dh: 0.,
                        rotation: 0.,
                    },
                    w,
                    cx,
                );
                cx.stop_propagation();
                return;
            }
            if command && key == "x" {
                self.clipboard_copy(true, w, cx);
                cx.stop_propagation();
                return;
            }
            if command && key == "a" {
                if let Some(e) = &mut self.editor {
                    let ids = e.session.model().slides[self.page]
                        .shapes
                        .iter()
                        .filter(|s| !s.hidden && !s.locked)
                        .map(|s| s.id)
                        .collect();
                    let _ = e.session.select(self.page, ids);
                }
                cx.stop_propagation();
                cx.notify();
                return;
            }
            if key == "enter" && selected {
                self.begin_selected_text(w, cx);
                cx.stop_propagation();
                return;
            }
            if command && key == "v" {
                self.paste_grid(w, cx);
                cx.stop_propagation();
            } else if command && key == "c" {
                self.clipboard_copy(false, w, cx);
                cx.stop_propagation();
            }
            return;
        }
        if self.composing() {
            if key == "escape" {
                self.cancel_inline(cx);
                cx.stop_propagation();
            }
            return;
        }
        if command && matches!(key, "s" | "z" | "y" | "q" | "o" | "n") {
            return;
        }
        if command && key == "enter" {
            self.end_inline(cx);
            cx.stop_propagation();
            return;
        }
        if key == "tab" {
            self.next_cell(mods.shift, w, cx);
            cx.stop_propagation();
            return;
        }
        if command && matches!(key, "b" | "i" | "u") {
            let props = self.run_props();
            let on = match key {
                "b" => props.bold == Some(true),
                "i" => props.italic == Some(true),
                _ => props.underline.as_deref().is_some_and(|s| s != "none"),
            };
            self.format_runs(
                |p| match key {
                    "b" => p.bold = Some(!on),
                    "i" => p.italic = Some(!on),
                    _ => p.underline = Some(if on { "none" } else { "sng" }.into()),
                },
                w,
                cx,
            );
            cx.stop_propagation();
            return;
        }
        let Some(i) = &mut self.inline else { return };
        let value = i.draft.body.text();
        let at = i.draft.cursor();
        match key {
            "escape" => self.end_inline(cx),
            "a" if command => {
                i.draft.typing_props = None;
                i.draft.selection = 0..value.len();
                i.draft.reversed = false;
                cx.notify();
            }
            "c" | "x" if command => {
                if !i.draft.selection.is_empty() {
                    let a = text::pos(&value, i.draft.selection.start);
                    let b = text::pos(&value, i.draft.selection.end);
                    let body = ed::slice(&i.draft.body, a, b);
                    let metadata =
                        serde_json::to_string(&serde_json::json!({"joydeck-text":1,"body":body}))
                            .unwrap_or_default();
                    cx.write_to_clipboard(ClipboardItem::new_string_with_metadata(
                        value[i.draft.selection.clone()].into(),
                        metadata,
                    ));
                    if key == "x" {
                        self.input_replace(None, "", false, None, w, cx);
                    }
                }
            }
            "v" if command => {
                if let Some(item) = cx.read_from_clipboard() {
                    self.paste_text(item, w, cx);
                }
            }
            "backspace" | "delete" => {
                if i.draft.selection.is_empty() {
                    i.draft.selection = if key == "backspace" {
                        text::previous(&value, at)..at
                    } else {
                        at..text::next(&value, at)
                    };
                }
                self.input_replace(None, "", false, None, w, cx);
            }
            "enter" => self.input_replace(
                None,
                if mods.shift { "\u{b}" } else { "\n" },
                false,
                None,
                w,
                cx,
            ),
            "left" | "right" | "up" | "down" | "home" | "end" => {
                let mut next = at;
                if command {
                    let (pi, ch) = text::pos(&value, at);
                    if let Some(para) = i.draft.body.paragraphs.get(pi) {
                        let line = para.text();
                        let position = if key == "left" {
                            joydeck_model::words::prev_stop(&line, ch)
                                .map(|ch| (pi, ch))
                                .or_else(|| {
                                    pi.checked_sub(1).and_then(|pi| {
                                        i.draft.body.paragraphs.get(pi).map(|p| (pi, p.char_len()))
                                    })
                                })
                        } else if key == "right" {
                            joydeck_model::words::next_stop(&line, ch)
                                .map(|ch| (pi, ch))
                                .or_else(|| {
                                    (pi + 1 < i.draft.body.paragraphs.len()).then_some((pi + 1, 0))
                                })
                        } else if key == "home" {
                            Some((0, 0))
                        } else if key == "end" {
                            i.draft
                                .body
                                .paragraphs
                                .last()
                                .map(|p| (i.draft.body.paragraphs.len() - 1, p.char_len()))
                        } else {
                            None
                        };
                        if let Some(position) = position {
                            next = text::byte_pos(&value, position);
                            i.draft.move_to(next, mods.shift);
                            w.notify_text_selection_changed(&self.focus);
                            cx.notify();
                            cx.stop_propagation();
                            return;
                        }
                    }
                }
                if let Some(p) = &i.placed {
                    let pos = text::pos(&value, at);
                    let pos = joydeck_text::Pos {
                        para: pos.0,
                        ch: pos.1,
                    };
                    let moved = match key {
                        "left" | "right" => {
                            let (pos, affinity) = p.layout.move_caret(
                                pos,
                                i.affinity,
                                key == "right",
                                joydeck_text::MoveMode::Visual,
                            );
                            i.affinity = affinity;
                            pos
                        }
                        "up" | "down" => p.layout.vertical(
                            pos,
                            key == "down",
                            p.layout.caret(pos).map_or(0., |c| c.0),
                        ),
                        "home" => p.layout.line_bounds(pos).0,
                        _ => p.layout.line_bounds(pos).1,
                    };
                    next = text::byte_pos(&value, (moved.para, moved.ch));
                }
                if !mods.shift && !i.draft.selection.is_empty() {
                    if key == "left" {
                        next = i.draft.selection.start;
                    } else if key == "right" {
                        next = i.draft.selection.end;
                    }
                }
                i.draft.move_to(next, mods.shift);
                w.notify_text_selection_changed(&self.focus);
                cx.notify();
            }
            _ => return,
        }
        w.notify_text_input_changed(&self.focus);
        cx.stop_propagation();
        cx.notify();
    }
    pub(super) fn next_cell(&mut self, back: bool, w: &mut Window, cx: &mut Context<Self>) {
        if self.composing() {
            return;
        }
        self.end_inline(cx);
        let Some((id, r, c)) = self.cell else { return };
        let Some(e) = &self.editor else { return };
        let Some(s) = e.session.model().slides[self.page].shape(id) else {
            return;
        };
        let ShapeKind::Table(t) = &s.kind else { return };
        let cells: Vec<_> = (0..t.n_rows())
            .flat_map(|r| (0..t.n_cols()).map(move |c| (r, c)))
            .filter(|(r, c)| t.cell(*r, *c).is_some_and(|c| !c.is_covered()))
            .collect();
        let at = cells.iter().position(|p| *p == (r, c)).unwrap_or(0);
        let next = if back {
            at.saturating_sub(1)
        } else {
            (at + 1).min(cells.len().saturating_sub(1))
        };
        if let Some((r, c)) = cells.get(next) {
            self.cell = Some((id, *r, *c));
            self.begin_selected_text(w, cx);
        }
    }
    pub(super) fn paste_grid(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        if self.inline.is_some() {
            if let Some(item) = cx.read_from_clipboard() {
                self.paste_text(item, w, cx);
            }
            return;
        }
        if let Some(item) = cx.read_from_clipboard()
            && let Some(metadata) = item.metadata().filter(|s| s.len() <= 32 * 1024 * 1024)
            && let Ok(transfer) =
                serde_json::from_str::<joydeck_engine::transfer::Transfer>(metadata)
        {
            self.dispatch(
                Command::PasteObjects {
                    transfer: Box::new(transfer),
                },
                w,
                cx,
            );
            return;
        }
        if let Some(item) = cx.read_from_clipboard()
            && let Some(bytes) = item.entries().iter().find_map(|e| {
                if let ClipboardEntry::Image(image) = e {
                    Some(image.bytes.clone())
                } else {
                    None
                }
            })
        {
            self.paste_image(bytes, cx);
            return;
        }
        let Some(value) = cx.read_from_clipboard().and_then(|i| i.text()) else {
            self.message = "剪贴板没有可用的文字、图片或 JoyDeck 对象".into();
            cx.notify();
            return;
        };
        if let Some((_, row, col)) = self.cell {
            self.dispatch(
                Command::Table {
                    action: TableAction::PasteTsv {
                        row,
                        col,
                        text: value,
                    },
                },
                w,
                cx,
            );
        } else {
            self.dispatch(
                Command::InsertText {
                    x: 80.,
                    y: 100.,
                    text: value,
                },
                w,
                cx,
            );
        }
    }
    pub(super) fn clipboard_copy(&mut self, cut: bool, w: &mut Window, cx: &mut Context<Self>) {
        if self.composing() {
            return;
        }
        if let Some(i) = &self.inline {
            let text = i.draft.body.text();
            let range = i.draft.selection.clone();
            if range.is_empty() {
                return;
            }
            let a = text::pos(&text, range.start);
            let b = text::pos(&text, range.end);
            let body = ed::slice(&i.draft.body, a, b);
            if let Ok(metadata) =
                serde_json::to_string(&serde_json::json!({"joydeck-text":1,"body":body}))
            {
                cx.write_to_clipboard(ClipboardItem::new_string_with_metadata(
                    text[range].into(),
                    metadata,
                ));
                if cut {
                    self.input_replace(None, "", false, None, w, cx);
                }
            }
            return;
        }
        let Some(e) = &self.editor else {
            return;
        };
        if cut && let Some(reason) = self.basic_edit_unavailable(BasicEdit::Delete, cx) {
            self.message = reason.to_string();
            cx.notify();
            return;
        }
        match joydeck_engine::transfer::capture(
            e.session.model(),
            self.page,
            &e.session.selection().shapes,
        )
        .and_then(|t| serde_json::to_string(&t).map_err(|e| e.to_string()))
        .and_then(|s| {
            if s.len() <= 32 * 1024 * 1024 {
                Ok(s)
            } else {
                Err("对象剪贴板超过 32 MiB".into())
            }
        }) {
            Ok(metadata) => {
                if cut {
                    let before = self.editor.as_ref().map(|e| e.session.revision());
                    self.run_basic_edit(BasicEdit::Delete, w, cx);
                    if self.editor.as_ref().map(|e| e.session.revision()) == before {
                        return;
                    }
                }
                cx.write_to_clipboard(ClipboardItem::new_string_with_metadata(
                    "JoyDeck 对象".into(),
                    metadata,
                ));
                self.message = if cut {
                    "已剪切对象"
                } else {
                    "已复制对象"
                }
                .into();
            }
            Err(e) => self.message = e,
        }
        cx.notify();
    }
    pub(super) fn copy_grid(&mut self, cx: &mut Context<Self>) {
        let Some(e) = &self.editor else { return };
        let Some(id) = e.session.selection().shapes.first() else {
            return;
        };
        let Some(s) = e.session.model().slides[self.page].shape(*id) else {
            return;
        };
        let value = if let ShapeKind::Table(t) = &s.kind {
            t.rows
                .iter()
                .map(|r| {
                    r.cells
                        .iter()
                        .map(|c| {
                            if c.is_covered() {
                                String::new()
                            } else {
                                c.text.text().replace(['\t', '\n'], " ")
                            }
                        })
                        .collect::<Vec<_>>()
                        .join("\t")
                })
                .collect::<Vec<_>>()
                .join("\n")
        } else {
            s.text.as_ref().map(|b| b.text()).unwrap_or_default()
        };
        if value.len() > 65536 {
            self.message = "复制内容超过 64 KiB".into();
        } else {
            cx.write_to_clipboard(ClipboardItem::new_string(value));
            self.message = "已复制文字/表格 TSV 到系统剪贴板".into();
        }
        cx.notify();
    }
}

impl EntityInputHandler for Viewer {
    fn text_for_range(
        &mut self,
        r: Range<usize>,
        actual: &mut Option<Range<usize>>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<String> {
        let t = self.inline.as_ref()?.draft.body.text();
        let r = text::utf16_range(&t, r)?;
        *actual = Some(text::to_utf16(&t, r.clone()));
        Some(t[r].into())
    }
    fn selected_text_range(
        &mut self,
        _: bool,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        let i = self.inline.as_ref()?;
        Some(UTF16Selection {
            range: text::to_utf16(&i.draft.body.text(), i.draft.selection.clone()),
            reversed: i.draft.reversed,
        })
    }
    fn set_selected_text_range(&mut self, r: Range<usize>, w: &mut Window, cx: &mut Context<Self>) {
        if let Some(i) = &mut self.inline
            && let Some(r) = text::utf16_range(&i.draft.body.text(), r)
        {
            i.draft.selection = r;
            i.draft.reversed = false;
            w.notify_text_selection_changed(&self.focus);
            cx.notify();
        }
    }
    fn text_length_utf16(&mut self, _: &mut Window, _: &mut Context<Self>) -> Option<usize> {
        Some(
            self.inline
                .as_ref()?
                .draft
                .body
                .text()
                .encode_utf16()
                .count(),
        )
    }
    fn marked_text_range(&self, _: &mut Window, _: &mut Context<Self>) -> Option<Range<usize>> {
        let i = self.inline.as_ref()?;
        Some(text::to_utf16(
            &i.draft.body.text(),
            i.draft.marked.clone()?,
        ))
    }
    fn unmark_text(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        if let Some(i) = &mut self.inline {
            i.draft.settle();
        }
        self.publish_inline(w, cx);
        w.notify_text_input_changed(&self.focus);
    }
    fn replace_text_in_range(
        &mut self,
        r: Option<Range<usize>>,
        s: &str,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.input_replace(r, s, false, None, w, cx);
    }
    fn replace_and_mark_text_in_range(
        &mut self,
        r: Option<Range<usize>>,
        s: &str,
        selected: Option<Range<usize>>,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.input_replace(r, s, true, selected, w, cx);
    }
    fn bounds_for_range(
        &mut self,
        r: Range<usize>,
        _: Bounds<Pixels>,
        w: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let i = self.inline.as_ref()?;
        let p = i.placed.as_ref()?;
        let value = i.draft.body.text();
        let r = text::utf16_range(&value, r)?;
        let pos = text::pos(&value, r.start);
        let (x, a, b) = p.layout.caret(joydeck_text::Pos {
            para: pos.0,
            ch: pos.1,
        })?;
        Some(self.screen_rect(p, joydeck_geom::Rect::new(x, a, x + 1., b), self.scale(w)))
    }
    fn character_index_for_point(
        &mut self,
        p: Point<Pixels>,
        w: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<usize> {
        let at = self.inline_hit(p, self.scale(w))?;
        Some(text::to_utf16(&self.inline.as_ref()?.draft.body.text(), at..at).start)
    }
}
