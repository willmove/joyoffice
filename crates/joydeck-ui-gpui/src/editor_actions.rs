//! GPUI adapter: document changes go through the headless transaction engine.
use super::*;
use joydeck_engine::Command;
use joydeck_model::{ShapeId, resolve};
pub(super) struct Drag {
    start: Point<Pixels>,
    current: Point<Pixels>,
    resize: bool,
    marquee: bool,
}
impl Viewer {
    pub(super) fn block_replace(&mut self, cx: &mut Context<Self>) -> bool {
        if self.saving || self.editor.as_ref().is_some_and(|e| e.session.dirty()) {
            self.message = "当前文稿有未保存修改或正在保存；请先保存，或使用“放弃修改”".into();
            cx.notify();
            true
        } else {
            false
        }
    }
    pub(super) fn sync_editor(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(editor) = &self.editor {
            self.page = editor.session.selection().page;
            match editor.display() {
                Ok(deck) => self.deck = Some(Arc::new(deck)),
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
        self.thumbnails.clear();
        self.thumbnail_errors.clear();
        self.thumb_busy = false;
        self.thumbs_cancel = Arc::new(AtomicBool::new(false));
        self.thumbs_task = None;
        self.drag = None;
        self.scroll.scroll_to_item(self.page, ScrollStrategy::Top);
        cx.notify();
    }
    fn checkpoint(&mut self, cx: &mut Context<Self>) {
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
                    Ok(()) => eprintln!("R2 checkpoint revision={revision}"),
                    Err(e) => {
                        s.message = format!("恢复快照未写入：{e}");
                        cx.notify();
                    }
                }
            });
        }));
    }
    fn dispatch(&mut self, command: Command, window: &mut Window, cx: &mut Context<Self>) {
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
                self.message = format!(
                    "修改已应用 · revision {} · Ctrl+S 保存",
                    editor.session.revision()
                );
                eprintln!(
                    "R2 command={label} revision={} dirty={}",
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
        if self.block_replace(cx) {
            return;
        }
        match joydeck_shell::editor::Editor::new(joydeck_shell::editor::recovery_root()) {
            Ok(editor) => {
                self.editor = Some(editor);
                self.source = None;
                self.generation += 1;
                self.save_input.update(cx, |s, cx| s.set_value("", w, cx));
                self.sync_editor(w, cx);
                self.checkpoint(cx);
                self.message = "新文稿 · 请指定 .joydeck 或 .pptx 保存路径".into();
            }
            Err(e) => self.message = e,
        }
        cx.notify();
    }
    pub(super) fn save(&mut self, _: &Save, w: &mut Window, cx: &mut Context<Self>) {
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
            self.close_after_save = false;
            self.message = "请在保存路径框填写新 .joydeck / .pptx 文件名".into();
            self.save_input.update(cx, |s, cx| s.focus(w, cx));
            cx.notify();
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
                                "R2 saved revision={revision} dirty={} path={}",
                                editor.session.dirty(),
                                path.display()
                            );
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
    fn discard_edits(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        if self.saving {
            return;
        }
        let view = cx.entity().downgrade();
        w.open_dialog(cx, move |dialog, _, _| {
            dialog
                .title("放弃未保存修改？")
                .child("此操作会清除当前会话的恢复快照。已保存文件不变。")
                .child(Button::new("confirm-discard").label("确认放弃").on_click({
                    let view = view.clone();
                    move |_, w, cx| {
                        let _ = view.update(cx, |s, cx| {
                            if let Some(e) = &s.editor {
                                let _ = e.recovery.discard();
                            }
                            s.editor = None;
                            w.close_dialog(cx);
                            if let Some(path) = s.source.clone() {
                                s.load(path, cx)
                            } else {
                                s.new_document(&NewDocument, w, cx)
                            }
                        });
                    }
                }))
        });
    }
    fn recover(&mut self, w: &mut Window, cx: &mut Context<Self>) {
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
            "R2 pointer position={:?} bounds={:?} point=({x},{y}) scale={scale}",
            event.position, self.canvas_bounds
        );
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
        } else if let Some(e) = &mut self.editor {
            let _ = e.session.select(self.page, vec![]);
        }
        self.drag = Some(Drag {
            start: event.position,
            current: event.position,
            resize: false,
            marquee: id.is_none(),
        });
        cx.notify();
    }
    pub(super) fn pointer_move(
        &mut self,
        event: &MouseMoveEvent,
        _scale: f32,
        cx: &mut Context<Self>,
    ) {
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
                if d.resize {
                    Command::Transform {
                        dx: 0.,
                        dy: 0.,
                        dw: dx,
                        dh: dy,
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
        let delta = self
            .drag
            .as_ref()
            .map(|d| {
                (
                    f32::from(d.current.x - d.start.x),
                    f32::from(d.current.y - d.start.y),
                    d.resize,
                )
            })
            .unwrap_or((0., 0., false));
        e.session
            .selection()
            .shapes
            .iter()
            .filter_map(|id| slide.shape(*id))
            .map(|s| {
                let b = resolve::xfrm(&ctx, s).bounds();
                div()
                    .absolute()
                    .left(px(b.x0 as f32 * scale + if delta.2 { 0. } else { delta.0 }))
                    .top(px(b.y0 as f32 * scale + if delta.2 { 0. } else { delta.1 }))
                    .w(px((b.width() as f32 * scale
                        + if delta.2 { delta.0 } else { 0. })
                    .max(1.)))
                    .h(px((b.height() as f32 * scale
                        + if delta.2 { delta.1 } else { 0. })
                    .max(1.)))
                    .border_2()
                    .border_color(rgb(color))
                    .into_any_element()
            })
            .collect()
    }
    pub(super) fn edit_toolbar(&self, _w: &mut Window, cx: &mut Context<Self>) -> Div {
        let disabled = self.editor.is_none() || self.loading;
        let selected = self
            .editor
            .as_ref()
            .is_some_and(|e| !e.session.selection().shapes.is_empty());
        let mut first = div().h(px(40.)).flex().items_center().gap_1().px_2();
        first = first.child(
            Button::new("new-document")
                .label("新建")
                .on_click(cx.listener(|s, _, w, cx| s.new_document(&NewDocument, w, cx))),
        );
        for (id, label, command, off) in [
            (
                "undo",
                "撤销",
                Command::Undo,
                !self.editor.as_ref().is_some_and(|e| e.session.can_undo()),
            ),
            (
                "redo",
                "重做",
                Command::Redo,
                !self.editor.as_ref().is_some_and(|e| e.session.can_redo()),
            ),
            (
                "rectangle",
                "矩形",
                Command::InsertRect {
                    x: 100.,
                    y: 260.,
                    width: 160.,
                    height: 90.,
                },
                false,
            ),
            ("duplicate", "复制", Command::Duplicate, !selected),
            ("layer-forward", "上移层", Command::BringForward, !selected),
            ("layer-backward", "下移层", Command::SendBackward, !selected),
            ("delete", "删除", Command::Delete, !selected),
            ("new-slide", "新页", Command::NewSlide, false),
            ("duplicate-slide", "复制页", Command::DuplicateSlide, false),
            ("delete-slide", "删页", Command::DeleteSlide, false),
        ] {
            first = first.child(
                Button::new(id)
                    .label(label)
                    .disabled(disabled || off)
                    .on_click(cx.listener(move |s, _, w, cx| s.dispatch(command.clone(), w, cx))),
            );
        }
        first = first.child(
            Button::new("next-object")
                .label("下一对象")
                .disabled(disabled)
                .on_click(cx.listener(|s, _, w, cx| {
                    if let Some(e) = &s.editor {
                        let shapes = &e.session.model().slides[s.page].shapes;
                        if !shapes.is_empty() {
                            let at = e
                                .session
                                .selection()
                                .shapes
                                .first()
                                .and_then(|id| shapes.iter().position(|sh| sh.id == *id))
                                .map_or(0, |i| (i + 1) % shapes.len());
                            let id = shapes[at].id;
                            s.select_object(id, false, w, cx);
                        }
                    }
                })),
        );
        let mut second = div()
            .h(px(40.))
            .flex()
            .items_center()
            .gap_1()
            .px_2()
            .child(
                div().w(px(80.)).text_xs().child(
                    self.editor
                        .as_ref()
                        .map(|e| format!("选中 {}", e.session.selection().shapes.len()))
                        .unwrap_or_default(),
                ),
            )
            .child(div().flex_1().min_w_0().child(Input::new(&self.text_input)))
            .child(
                Button::new("insert-text")
                    .label("插入文字")
                    .disabled(disabled)
                    .on_click(cx.listener(|s, _, w, cx| {
                        let text = s.text_input.read(cx).value().to_string();
                        s.dispatch(
                            Command::InsertText {
                                x: 80.,
                                y: 100.,
                                text: if text.is_empty() {
                                    "新文本".into()
                                } else {
                                    text
                                },
                            },
                            w,
                            cx,
                        )
                    })),
            )
            .child(
                Button::new("apply-text")
                    .label("应用文字")
                    .disabled(
                        disabled
                            || !self.editor.as_ref().is_some_and(|e| {
                                e.session.selection().shapes.len() == 1
                                    && e.session.model().slides[self.page]
                                        .shape(e.session.selection().shapes[0])
                                        .is_some_and(|s| {
                                            s.text.is_some()
                                                && matches!(s.kind, joydeck_model::ShapeKind::Shape)
                                        })
                            }),
                    )
                    .on_click(cx.listener(|s, _, w, cx| {
                        let text = s.text_input.read(cx).value().to_string();
                        s.dispatch(Command::ReplaceText { text }, w, cx)
                    })),
            );
        for (id, label, dx, dy, dw, dh, rotation) in [
            ("move-left", "←", -12., 0., 0., 0., 0.),
            ("move-right", "→", 12., 0., 0., 0., 0.),
            ("grow", "放大", 0., 0., 12., 8., 0.),
            ("shrink", "缩小", 0., 0., -12., -8., 0.),
            ("rotate", "旋转", 0., 0., 0., 0., 15.),
        ] {
            second = second.child(
                Button::new(id)
                    .label(label)
                    .disabled(disabled || !selected)
                    .on_click(cx.listener(move |s, _, w, cx| {
                        s.dispatch(
                            Command::Transform {
                                dx,
                                dy,
                                dw,
                                dh,
                                rotation,
                            },
                            w,
                            cx,
                        )
                    })),
            );
        }
        div()
            .flex_shrink_0()
            .flex()
            .flex_col()
            .child(first)
            .child(second)
            .child(
                div()
                    .h(px(40.))
                    .flex()
                    .items_center()
                    .gap_2()
                    .px_2()
                    .child(div().flex_1().min_w_0().child(Input::new(&self.save_input)))
                    .child(
                        Button::new("save-document")
                            .label(if self.saving {
                                "保存中"
                            } else {
                                "保存 / 另存"
                            })
                            .disabled(disabled || self.saving)
                            .on_click(cx.listener(|s, _, w, cx| s.save(&Save, w, cx))),
                    )
                    .child(
                        Button::new("recover-document")
                            .label("恢复")
                            .on_click(cx.listener(|s, _, w, cx| s.recover(w, cx))),
                    )
                    .child(
                        Button::new("discard-document")
                            .label("放弃修改")
                            .disabled(self.saving)
                            .on_click(cx.listener(|s, _, w, cx| s.discard_edits(w, cx))),
                    ),
            )
    }
}
