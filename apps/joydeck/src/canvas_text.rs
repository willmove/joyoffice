//! Canvas input adapter. Geometry is produced by the same ShapedLine used to paint.
use super::*;
use joydeck::text::{self, TextEdit};
use std::ops::Range;

pub(super) struct CanvasLine {
    start: usize,
    end: usize,
    shaped: ShapedLine,
    bounds: Bounds<Pixels>,
}
impl Editor {
    pub(super) fn begin_text(&mut self, i: usize, w: &mut Window, cx: &mut Context<Self>) {
        let mut o = self.effective_objects()[i].clone();
        o.runs = self.session.deck.slides[self.page].objects[i].runs.clone();
        if o.field.is_some() {
            self.message = "动态字段由文稿日期/页序解析，请先转换为普通文字再编辑".into();
            return;
        }
        if o.kind != "text" {
            return;
        }
        if o.rotation != 0. {
            self.message = "旋转文本请先将旋转设为 0 后编辑".into();
            return;
        }
        self.cell_edit = None;
        self.gesture = None;
        self.text_edit = Some(TextEdit::new(o));
        self.text_lines.clear();
        self.text_layout_key = None;
        self.visual_revision += 1;
        self.pending = None;
        self.image = None;
        self.focus.focus(w, cx);
        cx.notify();
    }
    pub(super) fn format_text(&mut self, kind: &str, w: &mut Window, cx: &mut Context<Self>) {
        let Some(t) = &mut self.text_edit else {
            return;
        };
        t.cancel();
        let mut style = t.typing.clone();
        match kind {
            "bold" => style.bold = !style.bold,
            "italic" => style.italic = !style.italic,
            "font" => {
                style.family = self.fields[12].read(cx).value().to_string();
                style.color = self.fields[13].read(cx).value().to_string();
                match self.author_fields[8].read(cx).value().parse::<f32>() {
                    Ok(size) => style.size = size,
                    Err(_) => {
                        self.message = "字号须为数字".into();
                        return;
                    }
                }
            }
            _ => {
                style.family = self.fields[12].read(cx).value().to_string();
                style.color = self.fields[13].read(cx).value().to_string();
            }
        }
        let mut draft = t.object.clone();
        if let Err(e) = text::format(&mut draft, t.selection.clone(), style.clone()) {
            self.message = e;
            cx.notify();
            return;
        }
        t.typing = style;
        t.object = draft;
        self.publish_text(cx);
        self.focus.focus(w, cx);
        cx.notify();
    }
    pub(super) fn publish_text(&mut self, cx: &mut Context<Self>) {
        let Some(t) = &self.text_edit else {
            return;
        };
        if t.marked.is_some() {
            cx.notify();
            return;
        }
        let object = t.object.clone();
        if let Some(target) = self.cell_edit.clone() {
            let mut runs = object.runs.clone();
            if let Some((o, table)) = self.table_target() {
                let inherited = joydeck::table::resolve(
                    &self.session.deck,
                    self.page,
                    table,
                    target.row,
                    target.col,
                )
                .text;
                if runs.iter().all(|r| r.style == inherited) {
                    runs.clear();
                }
                let _ = o;
            }
            match self.session.execute(
                self.session.id,
                Command::Table(
                    self.page,
                    target.id,
                    joydeck::table::Action::SetCell(target.row, target.col, object.text, runs),
                ),
            ) {
                Ok(_) => self.recover(cx),
                Err(e) => {
                    self.message = e;
                    self.text_edit = None;
                    self.cell_edit = None;
                }
            }
        } else if let Some(i) = self.session.deck.slides[self.page]
            .objects
            .iter()
            .position(|o| o.id == object.id)
        {
            if let Err(e) = self.session.execute(
                self.session.id,
                Command::ReplaceObject(self.page, i, object),
            ) {
                self.message = e;
                self.text_edit = None;
            } else {
                self.recover(cx);
            }
        }
        cx.notify();
    }
    pub(super) fn finish_text(&mut self, cx: &mut Context<Self>) {
        if let Some(t) = &mut self.text_edit {
            t.cancel();
        }
        self.publish_text(cx);
        self.cell_edit = None;
        self.scene_key = None;
        if self.text_edit.take().is_some() {
            self.text_lines.clear();
            self.text_layout_key = None;
            self.visual_revision += 1;
            self.pending = None;
            self.image = None;
            cx.notify();
        }
    }
    fn text_hit(&self, p: Point<Pixels>) -> usize {
        let Some(t) = &self.text_edit else {
            return 0;
        };
        let Some(line) = self.text_lines.iter().min_by(|a, b| {
            let distance = |l: &CanvasLine| {
                f32::from(l.bounds.top() - p.y).max(0.) + f32::from(p.y - l.bounds.bottom()).max(0.)
            };
            distance(a).total_cmp(&distance(b))
        }) else {
            return 0;
        };
        text::snap(
            &t.object.text,
            line.start
                + line
                    .shaped
                    .closest_index_for_x(p.x - line.bounds.left())
                    .min(line.end - line.start),
        )
    }
    pub(super) fn text_pointer(
        &mut self,
        e: &MouseDownEvent,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.text_edit.is_none() {
            return false;
        }
        let in_box = self.text_bounds.is_some_and(|b| b.contains(&e.position));
        if !in_box {
            self.finish_text(cx);
            return false;
        }
        let pos = self.text_hit(e.position);
        let Some(t) = self.text_edit.as_mut() else {
            return false;
        };
        if t.marked.is_some() {
            return true;
        }
        if e.click_count >= 3 {
            let start = t.object.text[..pos].rfind('\n').map_or(0, |i| i + 1);
            let end = t.object.text[pos..]
                .find('\n')
                .map_or(t.object.text.len(), |i| pos + i);
            t.selection = start..end;
        } else if e.click_count == 2 {
            use unicode_segmentation::UnicodeSegmentation;
            t.selection = t
                .object
                .text
                .split_word_bound_indices()
                .find(|(i, s)| *i <= pos && *i + s.len() > pos)
                .map_or(pos..pos, |(i, s)| i..i + s.len());
        } else if e.modifiers.shift {
            t.move_to(pos, true);
        } else {
            t.move_to(pos, false);
        }
        self.text_anchor = Some(t.selection.start);
        self.focus.focus(w, cx);
        w.notify_text_selection_changed(&self.focus);
        cx.notify();
        true
    }
    pub(super) fn text_move(&mut self, e: &MouseMoveEvent, cx: &mut Context<Self>) {
        if e.dragging()
            && let Some(anchor) = self.text_anchor
        {
            let end = self.text_hit(e.position);
            if let Some(t) = &mut self.text_edit {
                t.selection = anchor.min(end)..anchor.max(end);
                t.reversed = end < anchor;
                cx.notify();
            }
        }
    }
    pub(super) fn text_key(&mut self, e: &KeyDownEvent, w: &mut Window, cx: &mut Context<Self>) {
        if !self.focus.is_focused(w) || self.text_edit.is_none() {
            return;
        }
        let key = e.keystroke.key.as_str();
        let mods = e.keystroke.modifiers;
        if self
            .text_edit
            .as_ref()
            .is_some_and(|edit| edit.marked.is_some())
        {
            if key == "escape" {
                if let Some(edit) = &mut self.text_edit {
                    edit.cancel();
                }
                cx.stop_propagation();
                cx.notify();
            }
            return;
        }
        let command = mods.platform || mods.control;
        if command && matches!(key, "s" | "w" | "z" | "y") {
            self.finish_text(cx);
            return;
        }
        let Some(t) = self.text_edit.as_mut() else {
            return;
        };
        let at = t.cursor();
        let mut publish = false;
        match key {
            "escape" => {
                self.finish_text(cx);
            }
            "a" if command => t.selection = 0..t.object.text.len(),
            "c" | "x" if command => {
                if !t.selection.is_empty() {
                    cx.write_to_clipboard(ClipboardItem::new_string(
                        t.object.text[t.selection.clone()].to_string(),
                    ));
                    if key == "x" {
                        let _ = t.replace(None, "", false, None);
                        publish = true;
                    }
                }
            }
            "v" if command => {
                if let Some(value) = cx.read_from_clipboard().and_then(|i| i.text()) {
                    if let Err(e) = t.replace(
                        None,
                        &value.replace("\r\n", "\n").replace('\r', "\n"),
                        false,
                        None,
                    ) {
                        self.message = e;
                    }
                    publish = true;
                }
            }
            "b" | "i" if command => {
                if key == "b" {
                    t.typing.bold = !t.typing.bold;
                } else {
                    t.typing.italic = !t.typing.italic;
                }
                if !t.selection.is_empty() {
                    let _ = text::format(&mut t.object, t.selection.clone(), t.typing.clone());
                    publish = true;
                }
            }
            "backspace" | "delete" => {
                if t.selection.is_empty() {
                    t.selection = if key == "backspace" {
                        text::previous(&t.object.text, at)..at
                    } else {
                        at..text::next(&t.object.text, at)
                    };
                }
                let _ = t.replace(None, "", false, None);
                publish = true;
            }
            "enter" => {
                let _ = t.replace(None, "\n", false, None);
                publish = true;
            }
            "left" | "right" | "home" | "end" | "up" | "down" => {
                let pos = match key {
                    "left" => {
                        if !mods.shift && !t.selection.is_empty() {
                            t.selection.start
                        } else {
                            text::previous(&t.object.text, at)
                        }
                    }
                    "right" => {
                        if !mods.shift && !t.selection.is_empty() {
                            t.selection.end
                        } else {
                            text::next(&t.object.text, at)
                        }
                    }
                    "home" => t.object.text[..at].rfind('\n').map_or(0, |i| i + 1),
                    "end" => t.object.text[at..]
                        .find('\n')
                        .map_or(t.object.text.len(), |i| i + at),
                    "up" => t.object.text[..at].rfind('\n').unwrap_or(0),
                    _ => t.object.text[at..]
                        .find('\n')
                        .map_or(t.object.text.len(), |i| i + at + 1),
                };
                t.move_to(pos, mods.shift);
            }
            _ => return,
        }
        if publish {
            self.publish_text(cx);
        }
        w.notify_text_input_changed(&self.focus);
        cx.stop_propagation();
        cx.notify();
    }
    pub(super) fn paint_text(
        &mut self,
        bounds: Bounds<Pixels>,
        scale: f32,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(t) = &self.text_edit else {
            return;
        };
        let o = &t.object;
        let rect = Bounds::new(
            point(
                bounds.left() + px(o.x * scale),
                bounds.top() + px(o.y * scale),
            ),
            size(px(o.width * scale), px(o.height * scale)),
        );
        self.text_bounds = Some(rect);
        let height = px(o.font_size * scale * 1.3);
        let needs_layout = self
            .text_layout_key
            .as_ref()
            .is_none_or(|(old, bits, bounds)| {
                old != o || *bits != scale.to_bits() || *bounds != rect
            });
        if needs_layout {
            let mut lines = vec![];
            let mut offset = 0;
            for (n, value) in o.text.split('\n').enumerate() {
                let end = offset + value.len();
                let runs: Vec<_> = text::runs(o)
                    .into_iter()
                    .filter_map(|r| {
                        let len = r.end.min(end).saturating_sub(r.start.max(offset));
                        (len > 0).then(|| TextRun {
                            len,
                            font: Font {
                                family: if r.style.family == "sans-serif" {
                                    if cfg!(target_os = "macos") {
                                        "PingFang SC".into()
                                    } else {
                                        "Microsoft YaHei".into()
                                    }
                                } else {
                                    r.style.family.into()
                                },
                                weight: if r.style.bold {
                                    FontWeight::BOLD
                                } else {
                                    FontWeight::NORMAL
                                },
                                style: if r.style.italic {
                                    FontStyle::Italic
                                } else {
                                    FontStyle::Normal
                                },
                                ..Default::default()
                            },
                            color: rgb(u32::from_str_radix(&r.style.color[1..], 16).unwrap_or(0))
                                .into(),
                            background_color: None,
                            underline: None,
                            strikethrough: None,
                        })
                    })
                    .collect();
                self.text_shape_count += 1;
                let line = w.text_system().shape_line(
                    value.to_string().into(),
                    px(o.font_size * scale),
                    &runs,
                    None,
                );
                let line_bounds = Bounds::new(
                    point(
                        rect.left()
                            + if o.text_align == "center" {
                                (rect.size.width - line.width).max(px(0.)) / 2.
                            } else if o.text_align == "right" {
                                (rect.size.width - line.width).max(px(0.))
                            } else {
                                px(0.)
                            },
                        rect.top() + height * n,
                    ),
                    size(rect.size.width, height),
                );
                lines.push(CanvasLine {
                    start: offset,
                    end,
                    shaped: line,
                    bounds: line_bounds,
                });
                offset = end + 1;
            }
            self.text_lines = lines;
            self.text_layout_key = Some((o.clone(), scale.to_bits(), rect));
        }
        w.with_content_mask(Some(ContentMask { bounds: rect }), |w| {
            for line in &self.text_lines {
                let selected = t.selection.start.max(line.start)..t.selection.end.min(line.end);
                if selected.start < selected.end {
                    let x = line.shaped.x_for_index(selected.start - line.start);
                    let end = line.shaped.x_for_index(selected.end - line.start);
                    w.paint_quad(fill(
                        Bounds::new(
                            point(line.bounds.left() + x, line.bounds.top()),
                            size(end - x, height),
                        ),
                        rgba(0x2299ff55),
                    ));
                }
                let _ = line
                    .shaped
                    .paint(line.bounds.origin, height, TextAlign::Left, None, w, cx);
                if t.cursor() >= line.start && t.cursor() <= line.end && self.focus.is_focused(w) {
                    let x = line.shaped.x_for_index(t.cursor() - line.start);
                    w.paint_quad(fill(
                        Bounds::new(
                            point(line.bounds.left() + x, line.bounds.top()),
                            size(px(1.5), height),
                        ),
                        rgb(0x209fef),
                    ));
                }
                if let Some(marked) = &t.marked {
                    let start = marked.start.max(line.start);
                    let end = marked.end.min(line.end);
                    if start < end {
                        let x = line.shaped.x_for_index(start - line.start);
                        let end = line.shaped.x_for_index(end - line.start);
                        w.paint_quad(fill(
                            Bounds::new(
                                point(line.bounds.left() + x, line.bounds.bottom() - px(2.)),
                                size(end - x, px(1.)),
                            ),
                            rgb(0x209fef),
                        ));
                    }
                }
            }
        });
        w.handle_input(&self.focus, ElementInputHandler::new(rect, cx.entity()), cx);
    }
}
impl EntityInputHandler for Editor {
    fn text_for_range(
        &mut self,
        r: Range<usize>,
        actual: &mut Option<Range<usize>>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<String> {
        let t = self.text_edit.as_ref()?;
        let range = text::utf16_range(&t.object.text, r)?;
        *actual = Some(text::to_utf16(&t.object.text, range.clone()));
        Some(t.object.text[range].into())
    }
    fn selected_text_range(
        &mut self,
        _: bool,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        let t = self.text_edit.as_ref()?;
        Some(UTF16Selection {
            range: text::to_utf16(&t.object.text, t.selection.clone()),
            reversed: t.reversed,
        })
    }
    fn set_selected_text_range(&mut self, r: Range<usize>, w: &mut Window, cx: &mut Context<Self>) {
        if let Some(t) = &mut self.text_edit
            && let Some(r) = text::utf16_range(&t.object.text, r)
        {
            t.selection = r;
            t.reversed = false;
            w.notify_text_selection_changed(&self.focus);
            cx.notify();
        }
    }
    fn text_length_utf16(&mut self, _: &mut Window, _: &mut Context<Self>) -> Option<usize> {
        Some(self.text_edit.as_ref()?.object.text.encode_utf16().count())
    }
    fn marked_text_range(&self, _: &mut Window, _: &mut Context<Self>) -> Option<Range<usize>> {
        let t = self.text_edit.as_ref()?;
        Some(text::to_utf16(&t.object.text, t.marked.clone()?))
    }
    fn unmark_text(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        if let Some(t) = &mut self.text_edit {
            t.settle();
        }
        self.publish_text(cx);
        w.notify_text_input_changed(&self.focus);
    }
    fn replace_text_in_range(
        &mut self,
        r: Option<Range<usize>>,
        s: &str,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.ime_commit_calls += 1;
        if let Some(t) = &mut self.text_edit {
            match t.replace(r, s, false, None) {
                Ok(()) => self.publish_text(cx),
                Err(e) => self.message = e,
            }
        }
        w.notify_text_input_changed(&self.focus);
    }
    fn replace_and_mark_text_in_range(
        &mut self,
        r: Option<Range<usize>>,
        s: &str,
        selected: Option<Range<usize>>,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.ime_mark_calls += 1;
        if let Some(t) = &mut self.text_edit
            && let Err(e) = t.replace(r, s, true, selected)
        {
            self.message = e;
        }
        w.notify_text_input_changed(&self.focus);
        cx.notify();
    }
    fn bounds_for_range(
        &mut self,
        r: Range<usize>,
        _: Bounds<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let t = self.text_edit.as_ref()?;
        let r = text::utf16_range(&t.object.text, r)?;
        let l = self
            .text_lines
            .iter()
            .find(|l| r.start >= l.start && r.start <= l.end)?;
        let x = l.shaped.x_for_index(r.start - l.start);
        let end = l.shaped.x_for_index(r.end.min(l.end) - l.start);
        Some(Bounds::new(
            point(l.bounds.left() + x, l.bounds.top()),
            size((end - x).max(px(1.)), l.bounds.size.height),
        ))
    }
    fn character_index_for_point(
        &mut self,
        p: Point<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<usize> {
        let at = self.text_hit(p);
        Some(text::to_utf16(&self.text_edit.as_ref()?.object.text, at..at).start)
    }
}
