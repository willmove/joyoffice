//! One shell editor per open document, with a read snapshot and selection per window.
use super::*;
use joydeck_engine::{Command, Selection, Session};
use joydeck_model::{Presentation, SlideId};
use joydeck_shell::editor::{Editor, SaveJob, Saved};
use std::{cell::Cell, rc::Rc};

pub(super) struct SharedDocument {
    editor: Option<Editor>,
    identity: Option<PathBuf>,
    views: Rc<Cell<usize>>,
    saving: Rc<Cell<bool>>,
    checkpoint_task: Option<Task<()>>,
}

#[derive(Default)]
struct OpenDocuments(Vec<WeakEntity<SharedDocument>>);
impl Global for OpenDocuments {}

impl SharedDocument {
    pub(super) fn create(editor: Editor, identity: Option<PathBuf>, cx: &mut App) -> Entity<Self> {
        let document = cx.new(|_| Self {
            editor: Some(editor),
            identity,
            views: Rc::default(),
            saving: Rc::default(),
            checkpoint_task: None,
        });
        if !cx.has_global::<OpenDocuments>() {
            cx.set_global(OpenDocuments::default());
        }
        cx.update_global::<OpenDocuments, _>(|documents, _| {
            documents.0.retain(|entry| entry.upgrade().is_some());
            documents.0.push(document.downgrade());
        });
        document
    }

    pub(super) fn find(identity: &std::path::Path, cx: &App) -> Option<Entity<Self>> {
        cx.try_global::<OpenDocuments>()?
            .0
            .iter()
            .filter_map(WeakEntity::upgrade)
            .find(|document| {
                let shared = document.read(cx);
                shared.views.get() != 0 && shared.identity.as_deref() == Some(identity)
            })
    }

    pub(super) fn display(&self) -> Result<Deck, String> {
        self.editor.as_ref().ok_or("document is closed")?.display()
    }
}

pub(super) struct ViewSession {
    model: Arc<Presentation>,
    selection: Selection,
    revision: u64,
    dirty: bool,
    can_undo: bool,
    can_redo: bool,
}

impl ViewSession {
    fn new(session: &Session, selection: Selection) -> Self {
        Self {
            model: session.model_snapshot(),
            selection,
            revision: session.revision(),
            dirty: session.dirty(),
            can_undo: session.can_undo(),
            can_redo: session.can_redo(),
        }
    }
    pub(super) fn model(&self) -> &Presentation {
        &self.model
    }
    pub(super) fn model_snapshot(&self) -> Arc<Presentation> {
        self.model.clone()
    }
    pub(super) fn selection(&self) -> &Selection {
        &self.selection
    }
    pub(super) fn revision(&self) -> u64 {
        self.revision
    }
    pub(super) fn dirty(&self) -> bool {
        self.dirty
    }
    pub(super) fn can_undo(&self) -> bool {
        self.can_undo
    }
    pub(super) fn can_redo(&self) -> bool {
        self.can_redo
    }
    pub(super) fn select(&mut self, page: usize, shapes: Vec<ShapeId>) -> Result<(), String> {
        self.selection = Selection::checked(&self.model, page, shapes)?;
        Ok(())
    }
}

fn remap_selection(old: &ViewSession, model: &Presentation) -> Selection {
    let slide_id: Option<SlideId> = old
        .model
        .slides
        .get(old.selection.page)
        .map(|slide| slide.id);
    let same_page = slide_id.and_then(|id| model.slides.iter().position(|slide| slide.id == id));
    let page =
        same_page.unwrap_or_else(|| old.selection.page.min(model.slides.len().saturating_sub(1)));
    let shapes = same_page
        .map(|page| {
            old.selection
                .shapes
                .iter()
                .copied()
                .filter(|id| {
                    model.slides[page]
                        .shapes
                        .iter()
                        .any(|shape| shape.id == *id)
                })
                .collect()
        })
        .unwrap_or_default();
    Selection { page, shapes }
}

pub(super) struct ViewEditor {
    pub(super) document: Entity<SharedDocument>,
    pub(super) session: ViewSession,
    pub(super) source: Option<PathBuf>,
    views: Rc<Cell<usize>>,
    saving: Rc<Cell<bool>>,
}

impl Drop for ViewEditor {
    fn drop(&mut self) {
        self.views.set(self.views.get() - 1);
    }
}

impl ViewEditor {
    pub(super) fn attach(
        document: Entity<SharedDocument>,
        selection: Option<Selection>,
        cx: &App,
    ) -> Option<Self> {
        let core = document.read(cx);
        let editor = core.editor.as_ref()?;
        let selection = selection
            .and_then(|s| Selection::checked(editor.session.model(), s.page, s.shapes).ok())
            .unwrap_or_else(|| Selection {
                page: 0,
                shapes: Vec::new(),
            });
        let views = core.views.clone();
        views.set(views.get() + 1);
        Some(Self {
            document: document.clone(),
            session: ViewSession::new(&editor.session, selection),
            source: editor.source.clone(),
            views,
            saving: core.saving.clone(),
        })
    }

    pub(super) fn has_other_views(&self) -> bool {
        self.views.get() > 1
    }

    pub(super) fn saving(&self) -> bool {
        self.saving.get()
    }

    pub(super) fn refresh(&mut self, cx: &App) {
        if let Some(editor) = self.document.read(cx).editor.as_ref() {
            let selection = remap_selection(&self.session, editor.session.model());
            self.session = ViewSession::new(&editor.session, selection);
            self.source.clone_from(&editor.source);
        }
    }

    pub(super) fn execute(&mut self, command: Command, cx: &mut App) -> Result<bool, String> {
        let preserve_selection =
            self.has_other_views() && matches!(command, Command::Undo | Command::Redo);
        let (changed, session) = self.document.update(cx, |core, cx| {
            let editor = core.editor.as_mut().ok_or("document is closed")?;
            if editor.session.revision() != self.session.revision {
                return Err("文稿在另一窗口中更新，请重试".to_string());
            }
            let selection = &self.session.selection;
            editor
                .session
                .select(selection.page, selection.shapes.clone())?;
            let changed = editor.session.execute(command)?;
            let selection = if preserve_selection {
                remap_selection(&self.session, editor.session.model())
            } else {
                editor.session.selection().clone()
            };
            if changed {
                cx.notify();
            }
            Ok((changed, ViewSession::new(&editor.session, selection)))
        })?;
        self.session = session;
        Ok(changed)
    }

    pub(super) fn display(&self, cx: &App) -> Result<Deck, String> {
        self.document.read(cx).display()
    }

    pub(super) fn checkpoint_job(&self, cx: &App) -> Option<SaveJob> {
        self.document
            .read(cx)
            .editor
            .as_ref()
            .map(Editor::checkpoint_job)
    }

    pub(super) fn set_checkpoint_task(&self, task: Task<()>, cx: &mut App) {
        self.document
            .update(cx, |core, _| core.checkpoint_task = Some(task));
    }

    pub(super) fn begin_save(&self, path: PathBuf, cx: &mut App) -> Result<SaveJob, String> {
        if let Some(identity) = joydeck_shell::editor::canonical_source(&path)
            && SharedDocument::find(&identity, cx).is_some_and(|other| other != self.document)
        {
            return Err("目标文稿已在另一窗口打开".into());
        }
        self.document.update(cx, |core, cx| {
            if core.saving.get() {
                return Err("文稿正在另一窗口保存".into());
            }
            let editor = core.editor.as_ref().ok_or("document is closed")?;
            let job = editor.save_job(path)?;
            core.saving.set(true);
            cx.notify();
            Ok(job)
        })
    }

    pub(super) fn finish_save(&mut self, saved: Option<(Saved, Option<PathBuf>)>, cx: &mut App) {
        self.document.update(cx, |core, cx| {
            core.saving.set(false);
            if let Some((saved, identity)) = saved
                && let Some(editor) = &mut core.editor
            {
                editor.apply_saved(saved);
                core.identity = identity;
            }
            cx.notify();
        });
        self.refresh(cx);
    }

    pub(super) fn discard(self, cx: &mut App) {
        if self.has_other_views() {
            return;
        }
        if let Some(editor) = self.document.update(cx, |core, _| core.editor.take()) {
            cx.background_executor()
                .spawn(async move {
                    let _ = editor.recovery.discard();
                })
                .detach();
        }
    }

    pub(super) fn dispatch_attached(
        &mut self,
        dispatcher: &mut joydeck_shell::automation::Dispatcher,
        generation: u64,
        request: joydeck_shell::automation::Request,
        cx: &mut App,
    ) -> serde_json::Value {
        self.refresh(cx);
        let (response, selection) = self.document.update(cx, |core, cx| {
            if let Some(editor) = &mut core.editor {
                let selection = &self.session.selection;
                if let Err(message) = editor
                    .session
                    .select(selection.page, selection.shapes.clone())
                {
                    return (
                        serde_json::json!({
                            "requestId": request.request_id,
                            "error": {"code": "operation-failed", "message": message}
                        }),
                        None,
                    );
                }
            }
            let response = dispatcher.dispatch_attached(&mut core.editor, generation, request);
            let selection = core.editor.as_ref().map(|editor| {
                if self.session.revision != editor.session.revision()
                    || self.session.dirty != editor.session.dirty()
                    || self.source != editor.source
                {
                    cx.notify();
                }
                if self.source != editor.source {
                    core.identity = editor.source_identity();
                }
                editor.session.selection().clone()
            });
            (response, selection)
        });
        self.refresh(cx);
        if let Some(selection) = selection {
            self.session.selection = selection;
        }
        response
    }
}

impl Viewer {
    pub(super) fn is_saving(&self) -> bool {
        self.saving || self.editor.as_ref().is_some_and(ViewEditor::saving)
    }

    pub(super) fn requires_save_prompt(&self) -> bool {
        self.editor
            .as_ref()
            .is_some_and(|e| !e.has_other_views() && e.session.dirty())
    }

    pub(super) fn attach_document(
        &mut self,
        document: Entity<SharedDocument>,
        selection: Option<Selection>,
        discard_previous: bool,
        cx: &mut Context<Self>,
    ) {
        let Some(editor) = ViewEditor::attach(document.clone(), selection, cx) else {
            return;
        };
        self.source.clone_from(&editor.source);
        self.pending_save_path = Some(save_path(editor.source.as_deref()));
        if let Some(previous) = self.editor.replace(editor)
            && discard_previous
        {
            previous.discard(cx);
        }
        self.document_observer = Some(cx.observe(&document, |s, document, cx| {
            let Some(editor) = &mut s.editor else { return };
            if editor.document != document {
                return;
            }
            let revision = editor.session.revision();
            editor.refresh(cx);
            if s.source != editor.source {
                s.source.clone_from(&editor.source);
                s.pending_save_path = Some(save_path(editor.source.as_deref()));
                s.refresh_pending = true;
            }
            if revision != editor.session.revision() {
                s.page = editor.session.selection().page;
                s.refresh_pending = true;
                s.drag = None;
                s.render_generation += 1;
                if s.composing() {
                    s.message = "文稿已在另一窗口修改，当前组合输入已取消".into();
                }
            }
            cx.notify();
        }));
    }

    pub(super) fn attach_new_editor(&mut self, editor: Editor, cx: &mut Context<Self>) {
        let document = SharedDocument::create(editor, None, cx);
        self.attach_document(document, None, false, cx);
    }

    pub(super) fn new_shared_view(&mut self, cx: &mut Context<Self>) {
        let Some(editor) = &self.editor else { return };
        let document = editor.document.clone();
        let selection = editor.session.selection().clone();
        match open_window(self.dark, (1120., 800.), cx) {
            Ok((_, view)) => view.update(cx, |s, cx| {
                s.attach_document(document, Some(selection), false, cx);
                s.generation += 1;
                s.refresh_pending = true;
                s.message = "已打开当前文稿的新视图".into();
                cx.notify();
            }),
            Err(error) => {
                self.message = format!("无法打开文稿视图：{error}");
                cx.notify();
            }
        }
    }
}

fn save_path(path: Option<&std::path::Path>) -> String {
    path.filter(|p| {
        p.extension().is_some_and(|ext| {
            ext.eq_ignore_ascii_case("pptx") || ext.eq_ignore_ascii_case("joydeck")
        })
    })
    .map(|p| p.to_string_lossy().into_owned())
    .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::{Presentation, Session, ViewSession, remap_selection};
    use joydeck_engine::{Command, Selection, WorkspaceAction};

    #[test]
    fn peer_reorder_and_deletion_preserve_only_existing_view_targets() -> Result<(), String> {
        let mut session = Session::new(Presentation::default(), Default::default(), true);
        session.execute(Command::NewSlide)?;
        let view_page = session.selection().page;
        session.execute(Command::InsertRect {
            x: 8.,
            y: 8.,
            width: 20.,
            height: 20.,
        })?;
        let view = ViewSession::new(&session, session.selection().clone());
        let page_id = view.model.slides[view_page].id;
        assert_eq!(view.selection.shapes.len(), 1);
        session.execute(Command::Workspace {
            action: WorkspaceAction::ReorderSlides {
                slides: vec![page_id],
                before: Some(view.model.slides[0].id),
            },
        })?;
        let remapped = remap_selection(&view, session.model());
        assert_eq!(remapped.page, 0);
        assert_eq!(remapped.shapes, view.selection.shapes);
        session.select(remapped.page, remapped.shapes)?;
        session.execute(Command::DeleteSlide)?;
        assert_eq!(
            remap_selection(&view, session.model()),
            Selection {
                page: 0,
                shapes: Vec::new()
            }
        );
        Ok(())
    }
}
