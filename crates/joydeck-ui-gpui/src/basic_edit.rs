use super::*;
use joydeck_engine::{Command, WorkspaceAction};
use joydeck_model::SlideId;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum BasicEdit {
    Undo,
    Redo,
    NewSlide,
    DuplicateSlides,
    DeleteSlides,
    Duplicate,
    Delete,
    BringForward,
    SendBackward,
}

impl BasicEdit {
    pub(super) fn command(self, slides: Vec<SlideId>) -> Command {
        match self {
            Self::Undo => Command::Undo,
            Self::Redo => Command::Redo,
            Self::NewSlide => Command::NewSlide,
            Self::DuplicateSlides => Command::Workspace {
                action: WorkspaceAction::DuplicateSlides { slides },
            },
            Self::DeleteSlides => Command::Workspace {
                action: WorkspaceAction::DeleteSlides { slides },
            },
            Self::Duplicate => Command::Duplicate,
            Self::Delete => Command::Delete,
            Self::BringForward => Command::BringForward,
            Self::SendBackward => Command::SendBackward,
        }
    }

    fn slides(self, view: &Viewer) -> Vec<SlideId> {
        if matches!(self, Self::DuplicateSlides | Self::DeleteSlides) {
            view.selected_slides()
        } else {
            Vec::new()
        }
    }

    fn no_change(self, locale: joyoffice_ui::UiLocale) -> &'static str {
        let key = match self {
            Self::Undo => "edit.err_nothing_undo",
            Self::Redo => "edit.err_nothing_redo",
            Self::BringForward => "edit.err_cannot_forward",
            Self::SendBackward => "edit.err_cannot_backward",
            _ => "edit.err_no_change",
        };
        crate::l10n::t(locale, key)
    }
}

fn edit_reason(error: &str, locale: joyoffice_ui::UiLocale) -> &str {
    let key = match error {
        "select an object first" => "edit.err_select_object",
        "selected object is locked" => "edit.err_locked",
        "keep at least one slide" => "edit.err_keep_one_slide",
        "page budget exceeded" => "edit.err_page_budget",
        "shape budget exceeded" => "edit.err_shape_budget",
        "duplicate simple shapes only in R2" => "edit.err_duplicate_r2",
        "R2 duplicates simple slides; grouped/opaque/3D slides remain intact" => {
            "edit.err_duplicate_complex"
        }
        _ => return error,
    };
    crate::l10n::t(locale, key)
}

impl Viewer {
    pub(super) fn register_basic_actions(
        &self,
        mut root: Stateful<Div>,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        self.register_basic_action::<UndoEdit>(&mut root, BasicEdit::Undo, cx);
        self.register_basic_action::<RedoEdit>(&mut root, BasicEdit::Redo, cx);
        self.register_basic_action::<AddSlide>(&mut root, BasicEdit::NewSlide, cx);
        self.register_basic_action::<DuplicateSlides>(&mut root, BasicEdit::DuplicateSlides, cx);
        self.register_basic_action::<DeleteSlides>(&mut root, BasicEdit::DeleteSlides, cx);
        self.register_basic_action::<DuplicateObject>(&mut root, BasicEdit::Duplicate, cx);
        self.register_basic_action::<DeleteObject>(&mut root, BasicEdit::Delete, cx);
        self.register_basic_action::<MoveForward>(&mut root, BasicEdit::BringForward, cx);
        self.register_basic_action::<MoveBackward>(&mut root, BasicEdit::SendBackward, cx);
        root
    }

    fn register_basic_action<A: Action>(
        &self,
        root: &mut Stateful<Div>,
        edit: BasicEdit,
        cx: &mut Context<Self>,
    ) {
        if self.basic_edit_unavailable(edit, cx).is_none() {
            root.interactivity()
                .on_action(cx.listener(move |s, _: &A, w, cx| s.run_basic_edit(edit, w, cx)));
        }
    }

    fn basic_edit_block(
        &self,
        edit: BasicEdit,
        slides: &[SlideId],
        cx: &App,
    ) -> Option<SharedString> {
        if self.loading {
            return Some(self.t("edit.err_loading").into());
        }
        if self.benchmark.is_some() {
            return Some(self.t("edit.err_benchmark").into());
        }
        if self.composing() {
            return Some(self.t("edit.err_ime").into());
        }
        if self.drag.is_some() {
            return Some(self.t("edit.err_gesture").into());
        }
        let Some(editor) = &self.editor else {
            return Some(self.t("edit.err_no_doc").into());
        };
        match editor.validate_basic_edit(edit, slides, cx) {
            Ok(true) => None,
            Ok(false) => Some(edit.no_change(self.locale).into()),
            Err(error) => Some(edit_reason(&error, self.locale).to_owned().into()),
        }
    }

    pub(super) fn basic_edit_unavailable(&self, edit: BasicEdit, cx: &App) -> Option<SharedString> {
        self.basic_edit_block(edit, &edit.slides(self), cx)
    }

    pub(super) fn run_basic_edit(
        &mut self,
        edit: BasicEdit,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let slides = edit.slides(self);
        if let Some(reason) = self.basic_edit_block(edit, &slides, cx) {
            self.message = reason.to_string();
            cx.notify();
            return;
        }
        self.dispatch(edit.command(slides), w, cx);
    }

    pub(super) fn save_unavailable(&self) -> Option<&'static str> {
        if self.loading {
            Some(self.t("save.err_loading"))
        } else if self.save_picker_generation == Some(self.generation) {
            Some(self.t("save.err_picking"))
        } else if self.is_saving() {
            Some(self.t("save.err_in_progress"))
        } else if self.composing() {
            Some(self.t("save.err_ime"))
        } else if self.drag.is_some() {
            Some(self.t("edit.err_gesture"))
        } else if self.editor.is_none() {
            Some(self.t("edit.err_no_doc"))
        } else {
            None
        }
    }
}
