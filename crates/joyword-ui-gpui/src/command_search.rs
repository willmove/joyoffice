//! A searchable entry to desktop commands, styled after JoySheet's command palette.
use super::*;
use gpui::Focusable;
use gpui::component::{WindowExt, input::Input};

const COMMANDS: &[(&str, &str, &str)] = &[
    ("新建文档", "New document", "file.new"),
    ("打开文档", "Open document", "file.open"),
    ("保存", "Save", "file.save"),
    ("另存为", "Save as", "file.save_as"),
    ("撤销", "Undo", "edit.undo"),
    ("重做", "Redo", "edit.redo"),
    ("上一页", "Previous page", "view.previous_page"),
    ("下一页", "Next page", "view.next_page"),
    ("放大", "Zoom in", "view.zoom_in"),
    ("缩小", "Zoom out", "view.zoom_out"),
    ("实际大小", "Actual size", "view.zoom_reset"),
    ("切换外观", "Appearance", "view.theme"),
    ("界面语言", "Language", "view.language"),
];

impl JoyWord {
    pub(super) fn open_command_search(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        let input = self.command_search.clone();
        input.update(cx, |state, cx| state.set_value("", w, cx));
        let focus_input = input.clone();
        let owner = cx.weak_entity();
        let title = self.shell.tr("搜索命令", "Search commands");
        let labels: Vec<_> = COMMANDS
            .iter()
            .map(|&(zh, en, id)| (self.shell.tr(zh, en), zh, en, id))
            .collect();
        w.open_dialog(cx, move |dialog, _, cx| {
            let query = input.read(cx).value().to_lowercase();
            let owner = owner.clone();
            dialog.title(title).width(px(460.)).child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(Input::new(&input).aria_label(title))
                    .child(
                        div()
                            .id("search-command-results")
                            .max_h(px(340.))
                            .overflow_y_scroll()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .children(
                                labels
                                    .iter()
                                    .filter(|(_, zh, en, id)| {
                                        zh.contains(query.trim())
                                            || en.to_lowercase().contains(query.trim())
                                            || id.contains(query.trim())
                                    })
                                    .map(|&(label, _, _, id)| {
                                        let owner = owner.clone();
                                        Button::new(id).ghost().label(label).on_click(
                                            move |_, w, cx| {
                                                w.close_dialog(cx);
                                                let _ = owner.update(cx, |v, cx| {
                                                    v.file_page = false;
                                                    v.execute(id, w, cx);
                                                });
                                            },
                                        )
                                    }),
                            ),
                    ),
            )
        });
        focus_input.read(cx).focus_handle(cx).focus(w, cx);
    }
}
