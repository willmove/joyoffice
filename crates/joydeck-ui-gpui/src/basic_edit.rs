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

    fn no_change(self) -> &'static str {
        match self {
            Self::Undo => "没有可撤销的操作",
            Self::Redo => "没有可重做的操作",
            Self::BringForward => "选中对象无法再上移",
            Self::SendBackward => "选中对象无法再下移",
            _ => "此命令不会修改文稿",
        }
    }
}

fn edit_reason(error: &str) -> &str {
    match error {
        "select an object first" => "请先选择对象",
        "selected object is locked" => "选中对象已锁定",
        "keep at least one slide" => "至少保留一张幻灯片",
        "page budget exceeded" => "已达到幻灯片数量上限",
        "shape budget exceeded" => "已达到对象数量上限",
        "duplicate simple shapes only in R2" => "当前仅支持为基本形状、图片和表格创建副本",
        "R2 duplicates simple slides; grouped/opaque/3D slides remain intact" => {
            "当前页包含暂不支持复制的组合、未知或 3D 对象"
        }
        _ => error,
    }
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
            return Some("正在读取，请稍后编辑".into());
        }
        if self.benchmark.is_some() {
            return Some("性能对照进行中；完成后可以继续编辑".into());
        }
        if self.composing() {
            return Some("请先完成或取消输入法组合输入".into());
        }
        if self.drag.is_some() {
            return Some("请先结束当前手势".into());
        }
        let Some(editor) = &self.editor else {
            return Some("请先打开或新建文稿".into());
        };
        match editor.validate_basic_edit(edit, slides, cx) {
            Ok(true) => None,
            Ok(false) => Some(edit.no_change().into()),
            Err(error) => Some(edit_reason(&error).to_owned().into()),
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
            Some("正在读取，请稍后保存")
        } else if self.is_saving() {
            Some("文稿正在保存")
        } else if self.composing() {
            Some("请先完成或取消输入法组合输入再保存")
        } else if self.drag.is_some() {
            Some("请先结束当前手势")
        } else if self.editor.is_none() {
            Some("请先打开或新建文稿")
        } else {
            None
        }
    }
}
