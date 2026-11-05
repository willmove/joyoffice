//! Requests execute on the bound window's single editor, under the GUI busy guards.
use super::*;
pub struct Control {
    dispatcher: joydeck_shell::automation::Dispatcher,
    receive: std::sync::mpsc::Receiver<joydeck_shell::desktop_control::Pending>,
}
pub fn install(
    view: &Entity<Viewer>,
    handle: AnyWindowHandle,
    root: PathBuf,
    cx: &mut App,
) -> std::result::Result<(), String> {
    let dispatcher = joydeck_shell::automation::Dispatcher::new(&root)?;
    let receive = joydeck_shell::desktop_control::listen();
    handle
        .update(cx, |_, w, cx| {
            view.update(cx, |s, cx| {
                s.control = Some(Control {
                    dispatcher,
                    receive,
                });
                s.control_task = Some(cx.spawn_in(w, async move |view, cx| {
                    loop {
                        cx.background_executor()
                            .timer(std::time::Duration::from_millis(20))
                            .await;
                        if view
                            .update_in(cx, |s, w, cx| s.poll_control(w, cx))
                            .is_err()
                        {
                            break;
                        }
                    }
                }));
            })
        })
        .map_err(|e| e.to_string())?;
    Ok(())
}
impl Viewer {
    fn poll_control(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        let Some(mut control) = self.control.take() else {
            return;
        };
        if let Ok(pending) = control.receive.try_recv() {
            let previous_selection = self.editor.as_ref().map(|e| e.session.selection().clone());
            let previous_model = self.editor.as_ref().map(|e| e.session.model_snapshot());
            let composing = self.composing()
                || self
                    .notes_input
                    .update(cx, |i, cx| i.marked_text_range(w, cx).is_some())
                || self
                    .text_input
                    .update(cx, |i, cx| i.marked_text_range(w, cx).is_some());
            let response = if self.loading || self.is_saving() || composing || self.drag.is_some() {
                serde_json::json!({"requestId":pending.request.request_id,"error":{"code":"busy","message":"desktop is loading, saving, composing or dragging; retry with a new requestId"}})
            } else if let Some(editor) = &mut self.editor {
                editor.dispatch_attached(
                    &mut control.dispatcher,
                    self.generation,
                    pending.request.clone(),
                    cx,
                )
            } else {
                control.dispatcher.dispatch_attached(
                    &mut None,
                    self.generation,
                    pending.request.clone(),
                )
            };
            // The viewer can intentionally differ from the engine during inline preedit.
            // Only an actual engine change can invalidate that draft.
            let changed = self.editor.as_ref().is_some_and(|e| {
                previous_model
                    .as_ref()
                    .is_none_or(|before| !Arc::ptr_eq(before, &e.session.model_snapshot()))
            });
            let selection_changed =
                previous_selection != self.editor.as_ref().map(|e| e.session.selection().clone());
            if changed || selection_changed {
                self.sync_editor(w, cx);
                self.checkpoint(cx);
            }
            pending.respond(response);
            // Save and selection may change the title or controls without changing the model.
            if let Some(e) = &self.editor {
                self.source = e.source.clone();
                self.page = e.session.selection().page;
            }
            cx.notify();
        }
        self.control = Some(control);
    }
}
