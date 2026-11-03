//! Save-before-replace workflow; asynchronous saves never discard newer edits.
use super::*;
#[derive(Clone)]
pub(super) enum ReplaceDocument {
    New,
    Open,
    Path(PathBuf),
    Template(&'static str),
}
impl Viewer {
    pub(super) fn confirm_replace(
        &mut self,
        action: ReplaceDocument,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.composing()
            || self
                .notes_input
                .update(cx, |s, cx| s.marked_text_range(w, cx).is_some())
            || self.saving
        {
            return true;
        }
        self.commit_notes(w, cx);
        if !self.editor.as_ref().is_some_and(|e| e.session.dirty()) {
            return false;
        }
        let view = cx.entity().downgrade();
        w.open_dialog(cx, move |d, _, _| {
            let save = view.clone();
            let discard = view.clone();
            let save_action = action.clone();
            let discard_action = action.clone();
            d.title("是否保存对演示文稿的更改？")
                .child("保存后继续；取消可返回当前演示文稿。")
                .child(
                    div()
                        .flex()
                        .gap_2()
                        .child(Button::new("replace-save").label("保存").on_click(
                            move |_, w, cx| {
                                let _ = save.update(cx, |s, cx| {
                                    w.close_dialog(cx);
                                    s.pending_replace = Some(save_action.clone());
                                    s.save(&Save, w, cx);
                                });
                            },
                        ))
                        .child(Button::new("replace-discard").label("不保存").on_click(
                            move |_, w, cx| {
                                let _ = discard.update(cx, |s, cx| {
                                    if let Some(e) = s.editor.take() {
                                        let _ = e.recovery.discard();
                                    }
                                    w.close_dialog(cx);
                                    s.continue_replace(discard_action.clone(), w, cx);
                                });
                            },
                        ))
                        .child(
                            Button::new("replace-cancel")
                                .label("取消")
                                .on_click(|_, w, cx| w.close_dialog(cx)),
                        ),
                )
        });
        true
    }
    pub(super) fn continue_replace(
        &mut self,
        action: ReplaceDocument,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match action {
            ReplaceDocument::New => self.new_document(&NewDocument, w, cx),
            ReplaceDocument::Open => self.open(&Open, w, cx),
            ReplaceDocument::Path(path) => self.load(path, cx),
            ReplaceDocument::Template(name) => self.new_template(name, w, cx),
        }
    }
}
