//! GPUI adapter: document changes go through the headless transaction engine.
use super::*;
use joydeck_engine::Command;
use joydeck_model::{ShapeId, resolve};
pub(super) struct Drag {
    pub(super) start: Point<Pixels>,
    pub(super) current: Point<Pixels>,
    pub(super) resize: Option<(i8, i8)>,
    pub(super) marquee: bool,
}
impl Viewer {
    pub(super) fn block_replace(&mut self, cx: &mut Context<Self>) -> bool {
        if self.composing() {
            self.message = "请先完成或取消输入法组合输入".into();
            cx.notify();
            return true;
        }
        if self.saving || self.editor.as_ref().is_some_and(|e| e.session.dirty()) {
            self.message = "当前文稿有未保存更改或正在保存；请先保存后重试".into();
            cx.notify();
            true
        } else {
            false
        }
    }
    pub(super) fn sync_editor(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.end_inline(cx);
        let previous = self.deck.clone();
        if let Some(editor) = &self.editor {
            self.page = editor.session.selection().page;
            if self
                .cell
                .is_some_and(|(id, _, _)| !editor.session.selection().shapes.contains(&id))
            {
                self.cell = None;
            }
            match editor.display() {
                Ok(mut deck) => {
                    if editor.source.is_none() && deck.info.name == "Untitled.joydeck" {
                        deck.info.name = "演示文稿1".into();
                    }
                    self.deck = Some(Arc::new(deck));
                }
                Err(e) => self.message = e,
            }
            let text = editor
                .session
                .selection()
                .shapes
                .first()
                .and_then(|id| editor.session.model().slides[self.page].shape(*id))
                .and_then(|s| s.text.as_ref())
                .map(|t| t.text())
                .unwrap_or_default();
            self.text_input
                .update(cx, |s, cx| s.set_value(text, window, cx));
        }
        self.paint_cancel.store(true, Ordering::Relaxed);
        self.thumbs_cancel.store(true, Ordering::Relaxed);
        self.render_generation += 1;
        self.image = None;
        self.pending = None;
        let unchanged = |page: &usize| {
            previous
                .as_ref()
                .zip(self.deck.as_ref())
                .is_some_and(|(a, b)| {
                    a.presentation.masters == b.presentation.masters
                        && a.presentation.media == b.presentation.media
                        && a.presentation.slide_size == b.presentation.slide_size
                        && a.presentation.header_footer == b.presentation.header_footer
                        && a.presentation.default_text_style == b.presentation.default_text_style
                        && a.presentation
                            .slides
                            .get(*page)
                            .zip(b.presentation.slides.get(*page))
                            .is_some_and(|(a, b)| a == b)
                })
        };
        self.thumbnails.retain(|page, _| unchanged(page));
        self.thumbnail_errors.retain(unchanged);
        self.thumb_busy = false;
        self.thumbs_cancel = Arc::new(AtomicBool::new(false));
        self.thumbs_task = None;
        self.drag = None;
        self.scroll.scroll_to_item(self.page, ScrollStrategy::Top);
        cx.notify();
    }
    pub(super) fn checkpoint(&mut self, cx: &mut Context<Self>) {
        let Some(editor) = &self.editor else { return };
        let dirty = editor.session.dirty();
        let job = editor.checkpoint_job();
        let generation = self.generation;
        self.checkpoint_task = Some(cx.spawn(async move |view, cx| {
            cx.background_executor()
                .timer(std::time::Duration::from_millis(250))
                .await;
            let revision = job.revision;
            let result = cx
                .background_executor()
                .spawn(async move {
                    if dirty {
                        job.checkpoint()
                    } else {
                        job.recovery.clean(job.revision)
                    }
                })
                .await;
            let _ = view.update(cx, |s, cx| {
                if s.generation != generation {
                    return;
                }
                match result {
                    Ok(()) => eprintln!("R4/R5 checkpoint revision={revision}"),
                    Err(e) => {
                        s.message = format!("恢复快照未写入：{e}");
                        cx.notify();
                    }
                }
            });
        }));
    }
    pub(super) fn dispatch(
        &mut self,
        command: Command,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.benchmark.is_some() {
            self.message = "性能对照进行中；完成后可以继续编辑".into();
            cx.notify();
            return;
        }
        if self.composing() {
            self.message = "请先完成或取消输入法组合输入".into();
            cx.notify();
            return;
        }
        self.end_inline(cx);
        if self.loading {
            self.message = "正在读取，请稍后编辑".into();
            cx.notify();
            return;
        }
        let Some(editor) = &mut self.editor else {
            return;
        };
        let label = command.id();
        match editor.session.execute(command) {
            Ok(true) => {
                self.message = "就绪".into();
                eprintln!(
                    "R4/R5 command={label} revision={} dirty={}",
                    editor.session.revision(),
                    editor.session.dirty()
                );
                self.sync_editor(window, cx);
                self.checkpoint(cx);
            }
            Ok(false) => {}
            Err(e) => {
                self.message = e;
                cx.notify();
            }
        }
    }
    pub(super) fn undo_edit(&mut self, _: &UndoEdit, w: &mut Window, cx: &mut Context<Self>) {
        self.dispatch(Command::Undo, w, cx)
    }
    pub(super) fn redo_edit(&mut self, _: &RedoEdit, w: &mut Window, cx: &mut Context<Self>) {
        self.dispatch(Command::Redo, w, cx)
    }
    pub(super) fn delete_object(
        &mut self,
        _: &DeleteObject,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.dispatch(Command::Delete, w, cx)
    }
    pub(super) fn new_document(&mut self, _: &NewDocument, w: &mut Window, cx: &mut Context<Self>) {
        if self.confirm_replace(document_ui::ReplaceDocument::New, w, cx) {
            return;
        }
        match joydeck_shell::editor::Editor::new(joydeck_shell::editor::recovery_root()) {
            Ok(editor) => {
                self.editor = Some(editor);
                self.source = None;
                self.page = 0;
                self.zoom = None;
                self.cell = None;
                self.backstage = false;
                self.generation += 1;
                self.save_input.update(cx, |s, cx| s.set_value("", w, cx));
                self.sync_editor(w, cx);
                self.checkpoint(cx);
                self.message = "新演示文稿".into();
            }
            Err(e) => self.message = e,
        }
        cx.notify();
    }
    pub(super) fn save(&mut self, _: &Save, w: &mut Window, cx: &mut Context<Self>) {
        if self.composing()
            || self
                .notes_input
                .update(cx, |i, cx| i.marked_text_range(w, cx).is_some())
        {
            self.message = "请先完成或取消输入法组合输入再保存".into();
            cx.notify();
            return;
        }
        self.commit_notes(w, cx);
        self.end_inline(cx);
        if self.saving || self.loading {
            return;
        }
        let Some(editor) = &self.editor else { return };
        let text = self.save_input.read(cx).value().to_string();
        let path = if text.trim().is_empty() {
            editor
                .source
                .clone()
                .filter(|p| p.extension().is_some_and(|e| e == "pptx" || e == "joydeck"))
        } else {
            Some(PathBuf::from(text.trim().trim_matches('"')))
        };
        let Some(path) = path else {
            self.save_as(&SaveAs, w, cx);
            return;
        };
        let job = match editor.save_job(path) {
            Ok(j) => j,
            Err(e) => {
                self.message = e;
                cx.notify();
                return;
            }
        };
        let generation = self.generation;
        self.saving = true;
        self.message = "正在后台保存；可以继续编辑".into();
        self.save_task = Some(cx.spawn(async move |view, cx| {
            let result = cx
                .background_executor()
                .spawn(async move { job.run() })
                .await;
            let _ = view.update(cx, |s, cx| {
                if s.generation != generation {
                    return;
                }
                s.saving = false;
                match result {
                    Ok(saved) => {
                        let revision = saved.revision;
                        let path = saved.path.clone();
                        let warning = saved.warning.clone();
                        if let Some(editor) = &mut s.editor {
                            editor.apply_saved(saved);
                            if let Some(doc) = &mut s.deck {
                                Arc::make_mut(doc).info.name = path
                                    .file_name()
                                    .unwrap_or_default()
                                    .to_string_lossy()
                                    .into_owned();
                            }
                            s.source = Some(path.clone());
                            s.message = format!(
                                "已保存 revision {revision} · {}{}",
                                path.file_name().unwrap_or_default().to_string_lossy(),
                                if editor.session.dirty() {
                                    " · 仍有新修改未保存"
                                } else {
                                    ""
                                }
                            );
                            if let Some(warning) = warning {
                                s.message.push_str(&format!(" · 恢复清理提示：{warning}"));
                            }
                            eprintln!(
                                "R4/R5 saved revision={revision} dirty={} path={}",
                                editor.session.dirty(),
                                path.display()
                            );
                            if s.pending_replace.is_some() {
                                if editor.session.dirty() {
                                    s.pending_replace = None;
                                    s.message.push_str(" · 仍有新修改，请再次打开或新建");
                                } else {
                                    s.replace_ready = true;
                                }
                            }
                            if s.close_after_save && !editor.session.dirty() {
                                cx.quit();
                            } else if s.close_after_save {
                                s.close_after_save = false;
                                s.message.push_str(" · 有新修改，窗口继续打开");
                            }
                        }
                    }
                    Err(e) => {
                        s.message = format!("保存失败，原文件保持不变：{e}");
                        s.close_after_save = false;
                        s.pending_replace = None;
                    }
                }
                cx.notify();
            });
        }));
        cx.notify();
    }
    pub(super) fn request_close(
        &mut self,
        _: &CloseEditor,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.composing()
            || self
                .notes_input
                .update(cx, |i, cx| i.marked_text_range(w, cx).is_some())
        {
            self.message = "请先完成或取消组合输入".into();
            cx.notify();
            return;
        }
        self.commit_notes(w, cx);
        if self.saving {
            self.close_after_save = true;
            self.message = "正在保存，完成且无新修改后关闭".into();
            cx.notify();
            return;
        }
        if !self.editor.as_ref().is_some_and(|e| e.session.dirty()) {
            w.remove_window();
            return;
        }
        let weak = cx.entity().downgrade();
        let discard = weak.clone();
        let save = weak;
        w.open_dialog(cx, move |dialog, _, _| {
            dialog
                .title("关闭前保存文稿？")
                .child("当前修改尚未保存。取消可继续编辑。")
                .child(
                    div()
                        .flex()
                        .gap_2()
                        .child(Button::new("close-save").label("保存后关闭").on_click({
                            let view = save.clone();
                            move |_, w, cx| {
                                let _ = view.update(cx, |s, cx| {
                                    w.close_dialog(cx);
                                    s.close_after_save = true;
                                    s.save(&Save, w, cx);
                                });
                            }
                        }))
                        .child(Button::new("close-discard").label("放弃并关闭").on_click({
                            let view = discard.clone();
                            move |_, w, cx| {
                                let _ = view.update(cx, |s, cx| {
                                    if let Some(e) = &s.editor {
                                        let _ = e.recovery.discard();
                                    }
                                    w.close_dialog(cx);
                                    w.remove_window();
                                });
                            }
                        }))
                        .child(
                            Button::new("close-cancel")
                                .label("取消")
                                .on_click(|_, w, cx| w.close_dialog(cx)),
                        ),
                )
        });
    }
    pub(super) fn recover(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        if self.block_replace(cx) {
            return;
        }
        let root = joydeck_shell::editor::recovery_root();
        let view = cx.entity().downgrade();
        // Listing is bounded; full snapshot reads and lease checks run off the UI thread.
        self.dialog_task = Some(cx.spawn(async move |entity, cx| {
            let entries = cx
                .background_executor()
                .spawn(async move { joydeck_shell::storage::Recovery::new(root).list() })
                .await;
            let _ = entity.update(cx, |s, cx| {
                match entries {
                    Ok(rows) => {
                        s.recovery_rows = Some(rows);
                    }
                    Err(e) => s.message = e,
                }
                cx.notify();
            });
        }));
        let _ = (w, view);
    }
    pub(super) fn show_recoveries(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        let Some(rows) = self.recovery_rows.take() else {
            return;
        };
        let view = cx.entity().downgrade();
        w.open_dialog(cx, move |dialog, _, _| {
            dialog.title("可恢复文稿").child(
                div()
                    .id("recovery-list")
                    .max_h(px(360.))
                    .overflow_y_scroll()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(if rows.is_empty() {
                        "没有恢复记录"
                    } else {
                        "选择一个未被其它实例占用的快照"
                    })
                    .children(rows.iter().take(20).enumerate().map(|(i, e)| {
                        let record = e.record.clone();
                        let target = view.clone();
                        let disabled = e.active || e.problem.is_some();
                        Button::new(("recover-entry", i))
                            .label(format!(
                                "{} · revision {}{}",
                                e.record
                                    .source
                                    .as_ref()
                                    .and_then(|p| p.file_name())
                                    .map(|s| s.to_string_lossy().into_owned())
                                    .unwrap_or_else(|| "未命名".into()),
                                e.record.revision,
                                e.problem.as_deref().unwrap_or(if e.active {
                                    " · 正在使用"
                                } else {
                                    ""
                                })
                            ))
                            .disabled(disabled)
                            .on_click(move |_, w, cx| {
                                let record = record.clone();
                                let _ = target.update(cx, |s, cx| {
                                    if s.block_replace(cx) {
                                        return;
                                    }
                                    w.close_dialog(cx);
                                    let generation = s.generation;
                                    s.loading = true;
                                    s.read_task = Some(cx.spawn(async move |view, cx| {
                                        let result = cx
                                            .background_executor()
                                            .spawn(async move {
                                                joydeck_shell::editor::Editor::restore(
                                                    joydeck_shell::editor::recovery_root(),
                                                    &record,
                                                )
                                            })
                                            .await;
                                        let _ = view.update(cx, |s, cx| {
                                            if s.generation != generation {
                                                return;
                                            }
                                            s.loading = false;
                                            match result {
                                                Ok(editor) => {
                                                    s.editor = Some(editor);
                                                    s.source = None;
                                                    s.generation += 1;
                                                    s.refresh_pending = true;
                                                    s.pending_save_path = Some(String::new());
                                                    s.message =
                                                        "已恢复；请选择新路径保存，原文件不覆盖"
                                                            .into();
                                                }
                                                Err(e) => s.message = e,
                                            }
                                            cx.notify();
                                        });
                                    }));
                                });
                            })
                    })),
            )
        });
    }
    fn select_object(&mut self, id: ShapeId, extend: bool, w: &mut Window, cx: &mut Context<Self>) {
        let Some(e) = &mut self.editor else { return };
        let mut ids = if extend {
            e.session.selection().shapes.clone()
        } else {
            vec![]
        };
        if let Some(at) = ids.iter().position(|s| *s == id) {
            ids.remove(at);
        } else {
            ids.push(id);
        }
        let _ = e.session.select(self.page, ids);
        let text = e
            .session
            .selection()
            .shapes
            .first()
            .and_then(|id| e.session.model().slides[self.page].shape(*id))
            .and_then(|s| s.text.as_ref())
            .map(|t| t.text())
            .unwrap_or_default();
        self.text_input.update(cx, |s, cx| s.set_value(text, w, cx));
        cx.notify();
    }
    pub(super) fn pointer_down(
        &mut self,
        event: &MouseDownEvent,
        scale: f32,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.inline_pointer(event, scale, w, cx) {
            return;
        }
        self.focus.focus(w, cx);
        if self.loading {
            return;
        }
        let Some(e) = &self.editor else { return };
        let p = e.session.model();
        let slide = &p.slides[self.page];
        let Some(ctx) = resolve::Ctx::for_slide(p, slide) else {
            return;
        };
        let x = f32::from(event.position.x - self.canvas_bounds.origin.x) as f64 / scale as f64;
        let y = f32::from(event.position.y - self.canvas_bounds.origin.y) as f64 / scale as f64;
        eprintln!(
            "R4/R5 pointer position={:?} bounds={:?} point=({x},{y}) scale={scale}",
            event.position, self.canvas_bounds
        );
        for shape in slide
            .shapes
            .iter()
            .filter(|s| e.session.selection().shapes.contains(&s.id))
        {
            let b = resolve::xfrm(&ctx, shape).bounds();
            for (hx, hy) in [
                (-1, -1),
                (0, -1),
                (1, -1),
                (-1, 0),
                (1, 0),
                (-1, 1),
                (0, 1),
                (1, 1),
            ] {
                let px = b.x0 + (hx as f64 + 1.) * b.width() / 2.;
                let py = b.y0 + (hy as f64 + 1.) * b.height() / 2.;
                if ((x - px) * scale as f64).abs() <= 7. && ((y - py) * scale as f64).abs() <= 7. {
                    self.drag = Some(Drag {
                        start: event.position,
                        current: event.position,
                        resize: Some((hx, hy)),
                        marquee: false,
                    });
                    cx.notify();
                    return;
                }
            }
        }
        let id = slide
            .shapes
            .iter()
            .rev()
            .filter(|s| !s.hidden && !s.locked)
            .find(|s| {
                let t = resolve::xfrm(&ctx, s);
                let q = t.affine().inverse() * joydeck_geom::Point::new(x, y);
                q.x >= 0. && q.y >= 0. && q.x <= t.w && q.y <= t.h
            })
            .map(|s| s.id);
        if let Some(id) = id {
            if event.modifiers.shift || !e.session.selection().shapes.contains(&id) {
                self.select_object(id, event.modifiers.shift, w, cx);
            }
            self.cell = None;
            if let Some(e) = &self.editor {
                let p = e.session.model();
                let slide = &p.slides[self.page];
                if let Some(s) = slide.shape(id)
                    && let joydeck_model::ShapeKind::Table(t) = &s.kind
                    && let Some(ctx) = resolve::Ctx::for_slide(p, slide)
                {
                    let fields = joydeck_text::NoFields;
                    let point =
                        resolve::xfrm(&ctx, s).affine().inverse() * joydeck_geom::Point::new(x, y);
                    self.cell = joydeck_render::table_cell_rects(
                        t,
                        &joydeck_render::table_row_heights(&ctx, t, &fields),
                    )
                    .into_iter()
                    .find(|(_, _, rect)| rect.contains(point))
                    .map(|(r, c, _)| (id, r, c));
                }
            }
            let empty_placeholder = self
                .editor
                .as_ref()
                .and_then(|e| e.session.model().slides[self.page].shape(id))
                .is_some_and(|s| {
                    s.ph.is_some() && s.text.as_ref().is_some_and(|t| t.text().is_empty())
                });
            if event.click_count >= 2 || empty_placeholder {
                self.begin_selected_text(w, cx);
                return;
            }
        } else if let Some(e) = &mut self.editor {
            let _ = e.session.select(self.page, vec![]);
        }
        self.drag = Some(Drag {
            start: event.position,
            current: event.position,
            resize: None,
            marquee: id.is_none(),
        });
        cx.notify();
    }
    pub(super) fn pointer_move(
        &mut self,
        event: &MouseMoveEvent,
        scale: f32,
        cx: &mut Context<Self>,
    ) {
        if self.inline_move(event, scale, cx) {
            return;
        }
        if let Some(d) = &mut self.drag {
            d.current = event.position;
            cx.notify();
        }
    }
    pub(super) fn pointer_up(
        &mut self,
        event: &MouseUpEvent,
        scale: f32,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(d) = self.drag.take() else { return };
        let dx = f32::from(event.position.x - d.start.x) as f64 / scale as f64;
        let dy = f32::from(event.position.y - d.start.y) as f64 / scale as f64;
        if dx.abs() + dy.abs() < 1. {
            cx.notify();
            return;
        }
        if d.marquee {
            if let Some(e) = &mut self.editor {
                let p = e.session.model();
                let slide = &p.slides[self.page];
                if let Some(ctx) = resolve::Ctx::for_slide(p, slide) {
                    let a = joydeck_geom::Point::new(
                        f32::from(d.start.x - self.canvas_bounds.origin.x) as f64 / scale as f64,
                        f32::from(d.start.y - self.canvas_bounds.origin.y) as f64 / scale as f64,
                    );
                    let rect =
                        joydeck_model::Rect::from_points(a, a + joydeck_geom::Vec2::new(dx, dy));
                    let ids = slide
                        .shapes
                        .iter()
                        .filter(|s| {
                            !s.hidden
                                && !s.locked
                                && rect.intersect(resolve::xfrm(&ctx, s).bounds()).area() > 0.
                        })
                        .map(|s| s.id)
                        .collect();
                    let _ = e.session.select(self.page, ids);
                }
            }
            cx.notify();
        } else {
            self.dispatch(
                if let Some((hx, hy)) = d.resize {
                    Command::Transform {
                        dx: if hx < 0 { dx } else { 0. },
                        dy: if hy < 0 { dy } else { 0. },
                        dw: dx * hx as f64,
                        dh: dy * hy as f64,
                        rotation: 0.,
                    }
                } else {
                    Command::Transform {
                        dx,
                        dy,
                        dw: 0.,
                        dh: 0.,
                        rotation: 0.,
                    }
                },
                w,
                cx,
            );
        }
    }
    pub(super) fn selection_overlays(&self, scale: f32, color: u32) -> Vec<AnyElement> {
        let Some(e) = &self.editor else { return vec![] };
        let p = e.session.model();
        let slide = &p.slides[self.page];
        let Some(ctx) = resolve::Ctx::for_slide(p, slide) else {
            return vec![];
        };
        let (dx, dy, resize) = self
            .drag
            .as_ref()
            .map(|d| {
                (
                    f32::from(d.current.x - d.start.x),
                    f32::from(d.current.y - d.start.y),
                    d.resize,
                )
            })
            .unwrap_or((0., 0., None));
        let mut overlays = Vec::new();
        for shape in e
            .session
            .selection()
            .shapes
            .iter()
            .filter_map(|id| slide.shape(*id))
        {
            let b = resolve::xfrm(&ctx, shape).bounds();
            let (mx, my, dw, dh) = if let Some((hx, hy)) = resize {
                (
                    if hx < 0 { dx } else { 0. },
                    if hy < 0 { dy } else { 0. },
                    dx * hx as f32,
                    dy * hy as f32,
                )
            } else {
                (dx, dy, 0., 0.)
            };
            let (x, y, w, h) = (
                b.x0 as f32 * scale + mx,
                b.y0 as f32 * scale + my,
                (b.width() as f32 * scale + dw).max(1.),
                (b.height() as f32 * scale + dh).max(1.),
            );
            let mut outline = div()
                .absolute()
                .left(px(x))
                .top(px(y))
                .w(px(w))
                .h(px(h))
                .border_1()
                .border_color(rgb(color));
            for (hx, hy) in [
                (0., 0.),
                (0.5, 0.),
                (1., 0.),
                (0., 0.5),
                (1., 0.5),
                (0., 1.),
                (0.5, 1.),
                (1., 1.),
            ] {
                outline = outline.child(
                    div()
                        .absolute()
                        .left(px(hx * w - 3.5))
                        .top(px(hy * h - 3.5))
                        .w(px(7.))
                        .h(px(7.))
                        .bg(rgb(0xffffff))
                        .border_1()
                        .border_color(rgb(color))
                        .rounded(px(1.)),
                );
            }
            overlays.push(outline.into_any_element());
        }
        if let Some(d) = &self.drag
            && d.marquee
        {
            let x = f32::from(d.start.x.min(d.current.x) - self.canvas_bounds.left());
            let y = f32::from(d.start.y.min(d.current.y) - self.canvas_bounds.top());
            overlays.push(
                div()
                    .absolute()
                    .left(px(x))
                    .top(px(y))
                    .w(px(f32::from(d.current.x - d.start.x).abs()))
                    .h(px(f32::from(d.current.y - d.start.y).abs()))
                    .border_1()
                    .border_color(rgb(0xd95a1a))
                    .bg(rgba(0xd95a1a12))
                    .into_any_element(),
            );
        }
        overlays
    }
}
