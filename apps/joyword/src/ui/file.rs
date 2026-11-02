//! Native file workspace. Every enabled action uses the existing document jobs.
use super::{controls, theme};
use crate::*;
use joyoffice_ui::{IconName, icon};
use joyword_shell::exchange::Format;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum FilePage {
    Home,
    Open,
    Info,
    Print,
    Export,
    Options,
}
impl FilePage {
    fn key(self) -> &'static str {
        match self {
            Self::Home => "file.page.home",
            Self::Open => "file.page.open",
            Self::Info => "file.page.info",
            Self::Print => "file.page.print",
            Self::Export => "file.page.export",
            Self::Options => "file.page.options",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ExportAction {
    Command(CommandId),
    Text(Format),
}
pub(crate) const EXPORTS: &[(&str, &str, ExportAction)] = &[
    (
        "file-export-pdf",
        "file.export.pdf",
        ExportAction::Command(FILE_EXPORT_PDF),
    ),
    (
        "file-export-docx",
        "file.export.docx",
        ExportAction::Command(FILE_SAVE_COPY),
    ),
    (
        "file-export-rtf",
        "file.export.rtf",
        ExportAction::Text(Format::Rtf),
    ),
    (
        "file-export-html",
        "file.export.html",
        ExportAction::Text(Format::Html),
    ),
    (
        "file-export-markdown",
        "file.export.markdown",
        ExportAction::Text(Format::Markdown),
    ),
    (
        "file-export-txt",
        "file.export.txt",
        ExportAction::Text(Format::Txt),
    ),
    (
        "file-export-epub",
        "file.export.epub",
        ExportAction::Command(FILE_EXPORT_EPUB),
    ),
    (
        "file-export-pdfa",
        "file.export.pdfa",
        ExportAction::Command(FILE_EXPORT_PDFA),
    ),
];

fn export_reason(command: CommandId, reason: &'static str) -> &'static str {
    match reason {
        "input.format_compatibility"
            if matches!(command, FILE_EXPORT_PDF | FILE_EXPORT_PDFA | FILE_PRINT) =>
        {
            "file.export.layout_blocked"
        }
        "input.format_compatibility" => "file.export.conversion_blocked",
        "input.preview_unavailable" => "file.export.preview_blocked",
        _ => reason,
    }
}

impl JoyWord {
    pub(crate) fn show_file_page(
        &mut self,
        page: FilePage,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.editor.as_ref().is_some_and(|e| e.is_composing()) {
            return;
        }
        self.popup = None;
        self.file_page = Some(page);
        window.focus(&self.file_focus, cx);
        cx.notify();
    }
    pub(crate) fn dismiss_file_page(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.file_page = None;
        window.focus(&self.focus_handle, cx);
        cx.notify();
    }
    fn file_command(&mut self, command: CommandId, window: &mut Window, cx: &mut Context<Self>) {
        self.dismiss_file_page(window, cx);
        self.run(command, window, cx);
    }
    fn choose_export(&mut self, action: ExportAction, window: &mut Window, cx: &mut Context<Self>) {
        match action {
            ExportAction::Command(command) => self.file_command(command, window, cx),
            ExportAction::Text(format) => {
                if let Availability::Disabled(reason) = self.input_availability(FILE_EXPORT_TEXT) {
                    self.report_input_error(self.label(reason));
                    cx.notify();
                    return;
                }
                self.dismiss_file_page(window, cx);
                self.begin_text_exchange(format, window, cx);
            }
        }
    }
    pub(crate) fn file_workspace(
        &self,
        page: FilePage,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let mut nav = div()
            .id("file-navigation")
            .role(gpui::Role::TabList)
            .aria_label(self.label("menu.file"))
            .flex()
            .flex_col()
            .w(px(248.0))
            .flex_shrink_0()
            .h_full()
            .bg(rgb(0x3b57d8))
            .text_color(rgb(0xffffff))
            .py_4()
            .overflow_y_scroll();
        nav = nav.child(self.file_item(
            "file-back",
            self.label("file.page.back"),
            IconName::ArrowLeft,
            None,
            false,
            true,
            cx,
            |v, w, c| v.dismiss_file_page(w, c),
        ));
        for (id, selected, glyph) in [
            ("file-tab-home", FilePage::Home, IconName::House),
            ("file-tab-open", FilePage::Open, IconName::FolderOpen),
            ("file-tab-info", FilePage::Info, IconName::Info),
        ] {
            nav = nav.child(self.file_item(
                id,
                self.label(selected.key()),
                glyph,
                Some(selected),
                selected == page,
                true,
                cx,
                move |v, w, c| v.show_file_page(selected, w, c),
            ));
        }
        for (id, key, glyph, command) in [
            ("file-save", "file.page.save", IconName::Save, FILE_SAVE),
            (
                "file-save-copy",
                "file.page.save_as",
                IconName::Files,
                FILE_SAVE_COPY,
            ),
        ] {
            nav = nav.child(self.file_item(
                id,
                self.label(key),
                glyph,
                None,
                false,
                self.file_enabled(command),
                cx,
                move |v, w, c| v.file_command(command, w, c),
            ));
        }
        for (id, selected, glyph) in [
            ("file-tab-print", FilePage::Print, IconName::Printer),
            ("file-tab-export", FilePage::Export, IconName::FileOutput),
        ] {
            nav = nav.child(self.file_item(
                id,
                self.label(selected.key()),
                glyph,
                Some(selected),
                selected == page,
                true,
                cx,
                move |v, w, c| v.show_file_page(selected, w, c),
            ));
        }
        nav = nav
            .child(self.file_item(
                "file-close",
                self.label("file.page.close"),
                IconName::X,
                None,
                false,
                true,
                cx,
                |v, w, c| v.file_command(FILE_CLOSE, w, c),
            ))
            .child(self.file_item(
                "file-tab-options",
                self.label(FilePage::Options.key()),
                IconName::Settings,
                Some(FilePage::Options),
                page == FilePage::Options,
                true,
                cx,
                |v, w, c| v.show_file_page(FilePage::Options, w, c),
            ))
            .child(self.file_item(
                "file-quit",
                self.label("command.app.quit"),
                IconName::LogOut,
                None,
                false,
                true,
                cx,
                |v, w, c| v.file_command(QUIT, w, c),
            ));

        let mut content = div()
            .id("file-content")
            .role(gpui::Role::TabPanel)
            .aria_label(self.label(page.key()))
            .flex()
            .flex_col()
            .flex_1()
            .min_w_0()
            .h_full()
            .overflow_y_scroll()
            .px(px(50.0))
            .py(px(36.0))
            .gap_2()
            .child(
                div()
                    .text_size(px(32.0))
                    .mb_3()
                    .child(self.label(page.key())),
            );
        match page {
            FilePage::Export => {
                content = content.child(div().mb_3().child(self.label("file.export.description")));
                if matches!(
                    self.input_availability(FILE_EXPORT_TEXT),
                    Availability::Disabled("input.format_compatibility")
                ) {
                    content = content.child(
                        div()
                            .id("file-export-compatibility-notice")
                            .role(gpui::Role::Note)
                            .aria_label(self.label("file.export.compatibility_note"))
                            .max_w(px(680.0))
                            .p_3()
                            .mb_3()
                            .rounded_md()
                            .bg(rgb(theme::NOTICE))
                            .text_size(px(14.0))
                            .child(self.label("file.export.compatibility_note")),
                    );
                }
                for &(id, key, action) in EXPORTS {
                    let command = match action {
                        ExportAction::Command(c) => c,
                        ExportAction::Text(_) => FILE_EXPORT_TEXT,
                    };
                    content = content.child(self.file_action(
                        id,
                        self.label(key),
                        self.file_enabled(command),
                        cx,
                        move |v, w, c| v.choose_export(action, w, c),
                    ));
                    if let Some(reason) = self.file_disabled_reason(command) {
                        let reason = self.label(export_reason(command, reason));
                        content = content.child(
                            div()
                                .id(format!("{id}-reason"))
                                .role(gpui::Role::Note)
                                .aria_label(reason.clone())
                                .max_w(px(400.0))
                                .mb_2()
                                .text_size(px(13.0))
                                .text_color(rgb(theme::TEXT_MUTED))
                                .child(reason),
                        );
                    }
                }
                content = content.child(
                    div()
                        .text_sm()
                        .text_color(rgb(theme::TEXT_MUTED))
                        .child(self.label("file.export.limit")),
                );
            }
            FilePage::Home | FilePage::Info => {
                content = content
                    .child(
                        div()
                            .text_size(px(21.0))
                            .child(self.document_title().unwrap_or_else(|| "JoyWord".into())),
                    )
                    .child(
                        self.editor
                            .as_ref()
                            .and_then(|e| e.source_path())
                            .map(|p| p.display().to_string())
                            .unwrap_or_else(|| self.label("file.page.no_path")),
                    )
                    .child(
                        self.label(
                            if self
                                .editor
                                .as_ref()
                                .is_some_and(|e| e.has_unsaved_changes())
                            {
                                "input.dirty"
                            } else {
                                "file.page.saved"
                            },
                        ),
                    )
                    .child(self.file_action(
                        "file-home-open",
                        self.label("command.file.open"),
                        true,
                        cx,
                        |v, w, c| v.file_command(FILE_OPEN, w, c),
                    ))
                    .child(self.file_action(
                        "file-home-export",
                        self.label("file.page.export"),
                        true,
                        cx,
                        |v, w, c| v.show_file_page(FilePage::Export, w, c),
                    ))
                    .child(self.file_action(
                        "file-mail-merge",
                        self.label("command.file.mail_merge"),
                        true,
                        cx,
                        |v, w, c| v.file_command(FILE_MAIL_MERGE, w, c),
                    ));
            }
            FilePage::Open => {
                content = content
                    .child(self.file_action(
                        "file-browse",
                        self.label("command.file.open"),
                        true,
                        cx,
                        |v, w, c| v.file_command(FILE_OPEN, w, c),
                    ))
                    .child(self.label("file.page.recent"));
                if self.session.recent().is_empty() {
                    content = content.child(self.label("file.page.no_recent"));
                }
                for (i, path) in self.session.recent().iter().enumerate() {
                    let path = path.clone();
                    content = content.child(self.file_action(
                        format!("file-recent-{i}"),
                        path.display().to_string(),
                        true,
                        cx,
                        move |v, w, c| {
                            v.dismiss_file_page(w, c);
                            v.open_startup_path(path.clone(), w, c);
                        },
                    ));
                }
            }
            FilePage::Print => {
                content =
                    content
                        .child(self.label("file.page.print_detail"))
                        .child(self.file_action(
                            "file-print-pdf",
                            self.label("command.file.print"),
                            self.file_enabled(FILE_PRINT),
                            cx,
                            |v, w, c| v.file_command(FILE_PRINT, w, c),
                        ));
            }
            FilePage::Options => {
                content = content
                    .child(self.label("file.page.language"))
                    .child(self.file_action(
                        "file-language",
                        self.label("file.page.switch_language"),
                        true,
                        cx,
                        |v, w, c| v.run(SWITCH_LANGUAGE, w, c),
                    ))
                    .child(self.label("stage"));
            }
        }
        div()
            .id("file-workspace")
            .tab_group()
            .track_focus(&self.file_focus)
            .key_context("JoyWordFile")
            .flex()
            .size_full()
            .bg(rgb(0xf9f9f9))
            .text_color(rgb(0x262626))
            .text_size(px(16.0))
            .on_key_down(cx.listener(|_, event: &gpui::KeyDownEvent, window, cx| {
                match event.keystroke.key.as_str() {
                    "tab" if event.keystroke.modifiers.shift => window.focus_prev(cx),
                    "tab" => window.focus_next(cx),
                    "down" => window.focus_next(cx),
                    "up" => window.focus_prev(cx),
                    _ => return,
                }
                cx.stop_propagation();
            }))
            .child(nav)
            .child(content)
            .into_any_element()
    }
    fn file_enabled(&self, command: CommandId) -> bool {
        self.file_disabled_reason(command).is_none()
    }
    fn file_disabled_reason(&self, command: CommandId) -> Option<&'static str> {
        if let Availability::Disabled(reason) = self.input_availability(command) {
            return Some(reason);
        }
        if command == FILE_EXPORT_TEXT && self.exchange_cancel.is_some() {
            return Some("file.export.busy");
        }
        None
    }
    #[allow(clippy::too_many_arguments)]
    fn file_item(
        &self,
        id: &'static str,
        label: String,
        glyph: IconName,
        page: Option<FilePage>,
        selected: bool,
        enabled: bool,
        cx: &mut Context<Self>,
        action: impl Fn(&mut Self, &mut Window, &mut Context<Self>) + 'static,
    ) -> gpui::AnyElement {
        let action = Arc::new(action);
        let click = action.clone();
        controls::named_control(
            id,
            if page.is_some() {
                gpui::Role::Tab
            } else {
                gpui::Role::Button
            },
            label.clone(),
        )
        .when(page.is_some(), |e| e.aria_selected(selected))
        .a11y_synthetic_children(move |b| {
            if !enabled {
                b.parent_node().set_disabled();
            }
        })
        .when(enabled, |e| e.tab_index(0))
        .flex()
        .items_center()
        .h(px(52.0))
        .flex_shrink_0()
        .px_7()
        .gap_4()
        .bg(rgb(if selected { 0x6078e2 } else { 0x3b57d8 }))
        .text_color(rgb(if enabled { 0xffffff } else { 0xaeb9ed }))
        .when(enabled, |e| {
            e.cursor_pointer()
                .hover(|s| s.bg(rgb(0x506bdf)))
                .focus(|s| s.bg(rgb(0x6078e2)))
        })
        .on_click(cx.listener(move |v, _, w, c| {
            if enabled {
                click(v, w, c);
            }
        }))
        .on_key_down(cx.listener(move |v, e: &gpui::KeyDownEvent, w, c| {
            if enabled && matches!(e.keystroke.key.as_str(), "enter" | "space") {
                action(v, w, c);
                c.stop_propagation();
            }
        }))
        .child(icon(glyph, 20.0, if enabled { 0xffffff } else { 0xaeb9ed }))
        .child(label)
        .into_any_element()
    }
    fn file_action(
        &self,
        id: impl Into<gpui::SharedString>,
        label: String,
        enabled: bool,
        cx: &mut Context<Self>,
        action: impl Fn(&mut Self, &mut Window, &mut Context<Self>) + 'static,
    ) -> gpui::AnyElement {
        let action = Arc::new(action);
        let click = action.clone();
        controls::named_control(id, gpui::Role::Button, label.clone())
            .a11y_synthetic_children(move |b| {
                if !enabled {
                    b.parent_node().set_disabled();
                }
            })
            .when(enabled, |e| e.tab_index(0))
            .w_full()
            .max_w(px(400.0))
            .min_h(px(43.0))
            .flex_shrink_0()
            .flex()
            .items_center()
            .px_3()
            .py_2()
            .border_1()
            .border_color(rgb(0x969696))
            .rounded(px(4.0))
            .bg(rgb(0xffffff))
            .text_color(rgb(if enabled { 0x262626 } else { 0x9a9a9a }))
            .when(enabled, |e| {
                e.cursor_pointer()
                    .hover(|s| s.bg(rgb(theme::SELECTED)))
                    .focus(|s| s.border_color(rgb(theme::ACCENT)).bg(rgb(theme::SELECTED)))
            })
            .on_click(cx.listener(move |v, _, w, c| {
                if enabled {
                    click(v, w, c);
                }
            }))
            .on_key_down(cx.listener(move |v, e: &gpui::KeyDownEvent, w, c| {
                if enabled && matches!(e.keystroke.key.as_str(), "enter" | "space") {
                    action(v, w, c);
                    c.stop_propagation();
                }
            }))
            .child(label)
            .into_any_element()
    }
}
