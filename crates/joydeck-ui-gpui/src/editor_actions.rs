//! GPUI adapter: document changes go through the headless transaction engine.
use super::*;
use joydeck_engine::Command;
use joydeck_model::{ShapeId, resolve};
#[path = "pointer_geometry.rs"]
mod pointer_geometry;
use pointer_geometry::{Frame, GRIPS, ROTATION_OFFSET, Transform, grip};

enum Gesture {
    Marquee { extend: bool },
    Transform { frame: Frame, kind: Transform },
}

pub(super) struct Drag {
    start: Point<Pixels>,
    current: Point<Pixels>,
    canvas: Bounds<Pixels>,
    scale: f32,
    revision: u64,
    shift: bool,
    gesture: Gesture,
}

impl Drag {
    fn point(&self, position: Point<Pixels>) -> joydeck_geom::Point {
        page_point(position, self.canvas, self.scale)
    }

    fn delta(&self, position: Point<Pixels>) -> joydeck_geom::Vec2 {
        self.point(position) - self.point(self.start)
    }

    pub(super) fn move_preview(&self) -> Point<Pixels> {
        if matches!(
            self.gesture,
            Gesture::Transform {
                kind: Transform::Move,
                ..
            }
        ) {
            self.current - self.start
        } else {
            Point::default()
        }
    }
}

fn page_point(position: Point<Pixels>, canvas: Bounds<Pixels>, scale: f32) -> joydeck_geom::Point {
    let scale = f64::from(scale);
    joydeck_geom::Point::new(
        f64::from(f32::from(position.x - canvas.left())) / scale,
        f64::from(f32::from(position.y - canvas.top())) / scale,
    )
}

impl Viewer {
    pub(super) fn selected_bounds(&self) -> Option<joydeck_model::Rect> {
        let e = self.editor.as_ref()?;
        let p = e.session.model();
        let slide = p.slides.get(self.page)?;
        let ctx = resolve::Ctx::for_slide(p, slide)?;
        e.session
            .selection()
            .shapes
            .iter()
            .filter_map(|id| slide.shape(*id))
            .map(|s| resolve::xfrm(&ctx, s).bounds())
            .reduce(|a, b| a.union(b))
    }
    pub(super) fn block_replace(&mut self, cx: &mut Context<Self>) -> bool {
        if self.composing() {
            self.message = "请先完成或取消输入法组合输入".into();
            cx.notify();
            return true;
        }
        if self.is_saving() || self.requires_save_prompt() {
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
                .is_some_and(|(id, row, col)| !editor.session.selection().shapes.contains(&id) || !matches!(editor.session.model().slides[self.page].shape(id).map(|s| &s.kind), Some(joydeck_model::ShapeKind::Table(t)) if t.cell(row, col).is_some_and(|c| !c.is_covered())))
            {
                self.cell = None;
            }
            self.slide_selection
                .retain(|id| editor.session.model().slide(*id).is_some());
            if self.cell_range.is_some_and(|(id, range)| !editor.session.selection().shapes.contains(&id) || !matches!(editor.session.model().slides[self.page].shape(id).map(|s| &s.kind), Some(joydeck_model::ShapeKind::Table(t)) if range.end_row < t.n_rows() && range.end_col < t.n_cols())) { self.cell_range = None; }
            match editor.display(cx) {
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
        let Some(job) = editor.checkpoint_job(cx) else {
            return;
        };
        let generation = self.generation;
        let task = cx.spawn(async move |view, cx| {
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
        });
        editor.set_checkpoint_task(task, cx);
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
        match editor.execute(command, cx) {
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
                self.stop_read();
                self.attach_new_editor(editor, cx);
                self.page = 0;
                self.zoom = None;
                self.cell = None;
                self.backstage = false;
                self.slide_selection.clear();
                self.slide_anchor = None;
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
        self.save_to(None, w, cx);
    }
    pub(super) fn save_to(
        &mut self,
        selected: Option<PathBuf>,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
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
        if self.is_saving() || self.loading {
            return;
        }
        let Some(editor) = &self.editor else { return };
        let path = selected.or_else(|| {
            let text = self.save_input.read(cx).value().to_string();
            if text.trim().is_empty() {
                editor.source.clone().filter(|p| {
                    p.extension().is_some_and(|e| {
                        e.eq_ignore_ascii_case("pptx") || e.eq_ignore_ascii_case("joydeck")
                    })
                })
            } else {
                Some(PathBuf::from(text.trim().trim_matches('"')))
            }
        });
        let Some(path) = path else {
            self.save_as(&SaveAs, w, cx);
            return;
        };
        let job = match editor.begin_save(path, cx) {
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
                .spawn(async move {
                    job.run().map(|saved| {
                        let identity = joydeck_shell::editor::canonical_source(&saved.path);
                        (saved, identity)
                    })
                })
                .await;
            let _ = view.update(cx, |s, cx| {
                if s.generation != generation {
                    return;
                }
                s.saving = false;
                match result {
                    Ok((saved, identity)) => {
                        let revision = saved.revision;
                        let path = saved.path.clone();
                        let warning = saved.warning.clone();
                        if let Some(editor) = &mut s.editor {
                            editor.finish_save(Some((saved, identity)), cx);
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
                                s.close_ready = true;
                            } else if s.close_after_save {
                                s.close_after_save = false;
                                s.message.push_str(" · 有新修改，窗口继续打开");
                            }
                        }
                    }
                    Err(e) => {
                        if let Some(editor) = &mut s.editor {
                            editor.finish_save(None, cx);
                        }
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
        if !self.requires_save_prompt() {
            w.remove_window();
            return;
        }
        let generation = self.generation;
        let revision = self.editor.as_ref().map(|e| e.session.revision());
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
                                    if generation != s.generation || s.is_saving() {
                                        return;
                                    }
                                    s.close_after_save = true;
                                    s.save(&Save, w, cx);
                                });
                            }
                        }))
                        .child(Button::new("close-discard").label("放弃并关闭").on_click({
                            let view = discard.clone();
                            move |_, w, cx| {
                                let _ = view.update(cx, |s, cx| {
                                    w.close_dialog(cx);
                                    if generation != s.generation
                                        || s.is_saving()
                                        || revision
                                            != s.editor.as_ref().map(|e| e.session.revision())
                                    {
                                        return;
                                    }
                                    if let Some(editor) = s.editor.take() {
                                        editor.discard(cx);
                                    }
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
                                    let revision = s.editor.as_ref().map(|e| e.session.revision());
                                    let (request, cancel) = s.start_read();
                                    s.read_task = Some(cx.spawn(async move |view, cx| {
                                        let result = cx
                                            .background_executor()
                                            .spawn(async move {
                                                let editor =
                                                    joydeck_shell::editor::Editor::restore(
                                                        joydeck_shell::editor::recovery_root(),
                                                        &record,
                                                    )?;
                                                cancel.check()?;
                                                Ok::<_, String>(editor)
                                            })
                                            .await;
                                        let _ = view.update(cx, |s, cx| {
                                            if s.read_request != request {
                                                return;
                                            }
                                            s.loading = false;
                                            if revision
                                                != s.editor.as_ref().map(|e| e.session.revision())
                                            {
                                                s.message =
                                                    "当前文稿已变更，请重新选择恢复记录".into();
                                                cx.notify();
                                                return;
                                            }
                                            match result {
                                                Ok(editor) => {
                                                    s.attach_new_editor(editor, cx);
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
        self.select_objects(ids, w, cx);
    }

    fn select_objects(&mut self, ids: Vec<ShapeId>, w: &mut Window, cx: &mut Context<Self>) {
        let Some(e) = &mut self.editor else { return };
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
    fn selection_frame(&self) -> Option<Frame> {
        let editor = self.editor.as_ref()?;
        let model = editor.session.model();
        let slide = model.slides.get(self.page)?;
        let ctx = resolve::Ctx::for_slide(model, slide)?;
        Frame::new(
            editor
                .session
                .selection()
                .shapes
                .iter()
                .filter_map(|id| slide.shape(*id))
                .filter(|shape| !shape.hidden && !shape.locked)
                .map(|shape| resolve::xfrm(&ctx, shape)),
        )
    }

    fn start_drag(
        &mut self,
        event: &MouseDownEvent,
        scale: f32,
        gesture: Gesture,
        cx: &mut Context<Self>,
    ) {
        let Some(editor) = &self.editor else { return };
        self.drag = Some(Drag {
            start: event.position,
            current: event.position,
            canvas: self.canvas_bounds,
            scale,
            revision: editor.session.revision(),
            shift: event.modifiers.shift,
            gesture,
        });
        cx.notify();
    }

    pub(super) fn pointer_down(
        &mut self,
        event: &MouseDownEvent,
        scale: f32,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.drag = None;
        if self.inline_pointer(event, scale, w, cx) {
            return;
        }
        self.focus.focus(w, cx);
        if self.loading || self.benchmark.is_some() {
            return;
        }
        let Some(e) = &self.editor else { return };
        let p = e.session.model();
        let slide = &p.slides[self.page];
        let Some(ctx) = resolve::Ctx::for_slide(p, slide) else {
            return;
        };
        let point = page_point(event.position, self.canvas_bounds, scale);
        let scale_f64 = f64::from(scale);
        if let Some(frame) = self.selection_frame()
            && let Some(kind) = frame.handle(point, scale_f64)
        {
            self.start_drag(event, scale, Gesture::Transform { frame, kind }, cx);
            return;
        }
        let id = slide
            .shapes
            .iter()
            .rev()
            .filter(|s| !s.hidden && !s.locked)
            .find(|shape| pointer_geometry::hits(&ctx, shape, point, 3. / scale_f64))
            .map(|s| s.id);
        if let Some(id) = id {
            let extend_cell = event.modifiers.shift
                && (self.cell.is_some_and(|(shape, _, _)| shape == id)
                    || self.cell_range.is_some_and(|(shape, _)| shape == id));
            let prior_cell = self.cell;
            let prior_range = self.cell_range;
            if (event.modifiers.shift && !extend_cell)
                || !e.session.selection().shapes.contains(&id)
            {
                self.select_object(id, event.modifiers.shift, w, cx);
            }
            self.cell = None;
            self.cell_range = None;
            if let Some(e) = &self.editor {
                let p = e.session.model();
                let slide = &p.slides[self.page];
                if let Some(s) = slide.shape(id)
                    && let joydeck_model::ShapeKind::Table(t) = &s.kind
                    && let Some(ctx) = resolve::Ctx::for_slide(p, slide)
                {
                    let fields = joydeck_text::NoFields;
                    let point = resolve::xfrm(&ctx, s).affine().inverse() * point;
                    self.cell = joydeck_render::table_cell_rects(
                        t,
                        &joydeck_render::table_row_heights(&ctx, t, &fields),
                    )
                    .into_iter()
                    .find(|(_, _, rect)| rect.contains(point))
                    .map(|(r, c, _)| (id, r, c));
                    if extend_cell && let Some((_, row, col)) = self.cell {
                        let (r, c) = prior_range
                            .map(|(_, v)| (v.row, v.col))
                            .or_else(|| prior_cell.map(|(_, r, c)| (r, c)))
                            .unwrap_or((row, col));
                        self.cell_range = Some((
                            id,
                            joydeck_engine::CellRange {
                                row: r.min(row),
                                col: c.min(col),
                                end_row: r.max(row),
                                end_col: c.max(col),
                            },
                        ));
                        self.cell = None;
                        self.drag = None;
                        cx.notify();
                        return;
                    }
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
                self.select_object(id, false, w, cx);
                self.begin_selected_text(w, cx);
                return;
            }
            if !event.modifiers.shift
                && let Some(frame) = self.selection_frame()
            {
                self.start_drag(
                    event,
                    scale,
                    Gesture::Transform {
                        frame,
                        kind: Transform::Move,
                    },
                    cx,
                );
            }
        } else {
            self.start_drag(
                event,
                scale,
                Gesture::Marquee {
                    extend: event.modifiers.shift,
                },
                cx,
            );
        }
    }

    fn gesture_current(&self, drag: &Drag, w: &Window) -> bool {
        w.is_window_active()
            && !self.loading
            && !self.backstage
            && !self.sorter
            && drag.scale == self.scale(w)
            && drag.canvas == self.canvas_bounds
            && self
                .editor
                .as_ref()
                .is_some_and(|e| e.session.revision() == drag.revision)
    }

    pub(super) fn capture_gesture(&self, w: &mut Window, cx: &mut Context<Self>) {
        // Window capture phase runs even over other controls or outside the slide's hitbox.
        let move_listener = cx.listener(Self::pointer_move);
        w.on_mouse_event(move |event: &MouseMoveEvent, phase, w, cx| {
            if phase == DispatchPhase::Capture {
                move_listener(event, w, cx);
            }
        });
        let up_listener = cx.listener(Self::pointer_up);
        w.on_mouse_event(move |event: &MouseUpEvent, phase, w, cx| {
            if phase == DispatchPhase::Capture && event.button == MouseButton::Left {
                up_listener(event, w, cx);
            }
        });
    }

    fn pointer_move(&mut self, event: &MouseMoveEvent, w: &mut Window, cx: &mut Context<Self>) {
        let Some(drag) = &self.drag else { return };
        if !event.dragging() || !self.gesture_current(drag, w) {
            self.drag = None;
        } else if let Some(drag) = &mut self.drag {
            drag.current = event.position;
            drag.shift = event.modifiers.shift;
            cx.stop_propagation();
        }
        cx.notify();
    }

    fn pointer_up(&mut self, event: &MouseUpEvent, w: &mut Window, cx: &mut Context<Self>) {
        let Some(drag) = self.drag.take() else { return };
        cx.stop_propagation();
        cx.notify();
        if !self.gesture_current(&drag, w) {
            return;
        }
        let distance = event.position - drag.start;
        let moved = f32::from(distance.x).hypot(f32::from(distance.y)) >= 4.;
        match drag.gesture {
            Gesture::Marquee { extend } => {
                let mut ids = if extend {
                    let Some(editor) = &self.editor else { return };
                    editor.session.selection().shapes.clone()
                } else {
                    Vec::new()
                };
                if moved {
                    let Some(editor) = &self.editor else { return };
                    let model = editor.session.model();
                    let slide = &model.slides[self.page];
                    let Some(ctx) = resolve::Ctx::for_slide(model, slide) else {
                        return;
                    };
                    let rect = joydeck_geom::Rect::from_points(
                        drag.point(drag.start),
                        drag.point(event.position),
                    );
                    let mut included: BTreeSet<_> = ids.iter().copied().collect();
                    for shape in &slide.shapes {
                        if !shape.hidden
                            && !shape.locked
                            && pointer_geometry::intersects(resolve::xfrm(&ctx, shape), rect)
                            && included.insert(shape.id)
                        {
                            ids.push(shape.id);
                        }
                    }
                }
                self.cell = None;
                self.cell_range = None;
                self.select_objects(ids, w, cx);
            }
            Gesture::Transform { frame, kind } if moved => {
                let next = frame.preview(kind, drag.delta(event.position), event.modifiers.shift);
                if let Some(command) = frame.command(next) {
                    self.dispatch(command, w, cx);
                }
            }
            Gesture::Transform { .. } => {}
        }
    }

    pub(super) fn paint_selection_frame(&self, bounds: Bounds<Pixels>, scale: f32, w: &mut Window) {
        let Some(frame) = self.visible_selection_frame() else {
            return;
        };
        let to_window = |p: joydeck_geom::Point| {
            bounds.origin + point(px(p.x as f32 * scale), px(p.y as f32 * scale))
        };
        let mut path = PathBuilder::stroke(px(1.));
        path.move_to(to_window(grip(frame, -1, -1, 0.)));
        for (x, y) in [(1, -1), (1, 1), (-1, 1), (-1, -1)] {
            path.line_to(to_window(grip(frame, x, y, 0.)));
        }
        path.move_to(to_window(grip(frame, 0, -1, 0.)));
        path.line_to(to_window(grip(
            frame,
            0,
            -1,
            ROTATION_OFFSET / f64::from(scale),
        )));
        if let Ok(path) = path.build() {
            w.paint_path(path, rgb(0x7a7a7a));
        }
    }

    fn visible_selection_frame(&self) -> Option<joydeck_geom::Xfrm> {
        match &self.drag {
            Some(drag) => match drag.gesture {
                Gesture::Transform { frame, kind } => {
                    Some(frame.preview(kind, drag.delta(drag.current), drag.shift))
                }
                Gesture::Marquee { .. } => self.selection_frame().map(|frame| frame.bounds),
            },
            None => self.selection_frame().map(|frame| frame.bounds),
        }
    }

    pub(super) fn selection_overlays(&self, scale: f32, color: u32) -> Vec<AnyElement> {
        let Some(e) = &self.editor else { return vec![] };
        let p = e.session.model();
        let slide = &p.slides[self.page];
        let Some(ctx) = resolve::Ctx::for_slide(p, slide) else {
            return vec![];
        };
        let mut overlays = Vec::new();
        for shape in slide
            .shapes
            .iter()
            .filter(|s| e.session.selection().shapes.contains(&s.id))
        {
            if let joydeck_model::ShapeKind::Table(t) = &shape.kind {
                let range = self
                    .cell_range
                    .filter(|(id, _)| *id == shape.id)
                    .map(|(_, r)| r)
                    .or_else(|| {
                        self.cell
                            .filter(|(id, _, _)| *id == shape.id)
                            .map(|(_, row, col)| joydeck_engine::CellRange {
                                row,
                                col,
                                end_row: row,
                                end_col: col,
                            })
                    });
                if let Some(range) = range {
                    let transform = resolve::xfrm(&ctx, shape).affine();
                    let heights =
                        joydeck_render::table_row_heights(&ctx, t, &joydeck_text::NoFields);
                    for (row, col, rect) in joydeck_render::table_cell_rects(t, &heights) {
                        if row >= range.row
                            && row <= range.end_row
                            && col >= range.col
                            && col <= range.end_col
                        {
                            let b = transform.transform_rect_bbox(rect);
                            overlays.push(
                                div()
                                    .absolute()
                                    .left(px(b.x0 as f32 * scale))
                                    .top(px(b.y0 as f32 * scale))
                                    .w(px(b.width() as f32 * scale))
                                    .h(px(b.height() as f32 * scale))
                                    .bg(rgba(0x0078d420))
                                    .border_1()
                                    .border_color(rgb(0x0078d4))
                                    .into_any_element(),
                            );
                        }
                    }
                }
            }
        }
        if let Some(frame) = self.visible_selection_frame() {
            for (x, y) in GRIPS {
                let p = grip(frame, x, y, 0.);
                overlays.push(
                    div()
                        .absolute()
                        .left(px(p.x as f32 * scale - 3.5))
                        .top(px(p.y as f32 * scale - 3.5))
                        .w(px(7.))
                        .h(px(7.))
                        .bg(rgb(0xffffff))
                        .border_1()
                        .border_color(rgb(color))
                        .rounded(px(1.))
                        .into_any_element(),
                );
            }
            let p = grip(frame, 0, -1, ROTATION_OFFSET / f64::from(scale));
            overlays.push(
                div()
                    .absolute()
                    .left(px(p.x as f32 * scale - 5.))
                    .top(px(p.y as f32 * scale - 5.))
                    .w(px(10.))
                    .h(px(10.))
                    .rounded_full()
                    .bg(rgb(0xffffff))
                    .border_1()
                    .border_color(rgb(color))
                    .into_any_element(),
            );
        }
        if let Some(d) = &self.drag
            && matches!(d.gesture, Gesture::Marquee { .. })
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
