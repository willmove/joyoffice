mod advanced;
#[cfg(feature = "agent")]
mod agent;
#[cfg(feature = "agent")]
mod agent_port;
mod command_search;
mod control;
mod document_view;
mod export;
mod formatting;
mod forms;
mod gestures;
mod input;
mod objects;
mod page_index;
mod page_setup;
mod perf;
#[cfg(windows)]
mod printing;
mod save;
mod search;
mod styles;
mod text_accessibility;
use document_view::{DocumentCanvas, OpenSource};
// GPUI desktop reader, sharing JoyOffice visual components.
use gpui::component::button::{Button, ButtonVariants};
use gpui::component::{Disableable, Selectable, Sizable};
use gpui::prelude::*;
use gpui::{
    App, Bounds, Context, FocusHandle, KeyBinding, Menu, MenuItem, Render, ScrollHandle,
    SharedString, Window, WindowBounds, WindowOptions, actions, div, px, rgb, size,
};
use gpui_kit as gpui;
use joyoffice_ui::{Brand, Palette, metrics};
use joyword_shell::Shell;

actions!(
    joyword,
    [
        SearchCommands,
        FindDocument,
        ReplaceDocument,
        GoToPage,
        Quit,
        NewDocument,
        OpenDocument,
        SaveDocument,
        SaveAsDocument,
        Undo,
        Redo,
        SelectAll,
        Copy,
        Cut,
        Paste,
        ZoomIn,
        ZoomOut,
        ZoomReset,
        ToggleTheme,
        ToggleLanguage,
        DocumentTab,
        DocumentBackTab
    ]
);

struct JoyWord {
    shell: Shell,
    focus: FocusHandle,
    scroll: ScrollHandle,
    tab: usize,
    file_page: bool,
    about: bool,
    canvas: DocumentCanvas,
    input: Option<gpui::Entity<input::InputBridge>>,
    page_bounds: std::collections::BTreeMap<usize, Bounds<gpui::Pixels>>,
    dragging: bool,
    confirm_discard: bool,
    replace_confirm: Option<(u64, u64)>,
    pending_open: Option<OpenSource>,
    legacy_recovery: Option<std::path::PathBuf>,
    close_pending: bool,
    save_task: Option<gpui::Task<()>>,
    saving: bool,
    #[cfg(feature = "agent")]
    agent: agent_port::LocalAgent,
    #[cfg(windows)]
    printing: bool,
    caret_on: bool,
    blink: Option<gpui::Task<()>>,
    subscriptions: Vec<gpui::Subscription>,
    viewport_bounds: Option<Bounds<gpui::Pixels>>,
    reveal_input: bool,
    font_picker: formatting::FontSelect,
    format_state: Option<joyword_shell::formatting::FormatState>,
    font_document: Option<u64>,
    styles_open: bool,
    style_pick: Option<String>,
    style_catalog: Vec<joyword_shell::authoring::StyleInfo>,
    style_catalog_key: Option<(u64, u64)>,
    style_search: gpui::Entity<gpui::component::input::InputState>,
    command_search: gpui::Entity<gpui::component::input::InputState>,
    asset_task: Option<gpui::Task<()>>,
    asset_epoch: u64,
    gesture: Option<gestures::Gesture>,
    advanced: advanced::Advanced,
}

impl JoyWord {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus = cx.focus_handle();
        window.focus(&focus, cx);
        let font_picker = formatting::initial_font_picker(window, cx);
        let font_subscription = cx.subscribe_in(&font_picker, window, |v, _, event, w, cx| {
            if let gpui::component::select::SelectEvent::Confirm(Some(family)) = event {
                v.run_format(
                    "format.font",
                    &serde_json::json!({"name":family.to_string(), "eastAsia":family.to_string()}),
                    w,
                    cx,
                );
            }
        });
        let style_search = cx.new(|cx| {
            gpui::component::input::InputState::new(window, cx).placeholder("搜索全部样式")
        });
        let style_subscription = cx.subscribe(
            &style_search,
            |_, _, _: &gpui::component::input::InputEvent, cx| cx.notify(),
        );

        let command_search = cx.new(|cx| {
            gpui::component::input::InputState::new(window, cx)
                .placeholder("搜索命令 / Search commands")
        });
        let command_subscription = cx.subscribe(
            &command_search,
            |_, _, _: &gpui::component::input::InputEvent, cx| cx.notify(),
        );
        let shell = Shell::default();
        if shell.dark {
            gpui::component::Theme::change(gpui::component::ThemeMode::Dark, Some(window), cx);
        }
        JoyWord {
            shell,
            focus,
            scroll: ScrollHandle::new(),
            tab: 0,
            file_page: false,
            about: false,
            canvas: DocumentCanvas::default(),
            input: None,
            page_bounds: Default::default(),
            dragging: false,
            confirm_discard: false,
            replace_confirm: None,
            pending_open: None,
            legacy_recovery: None,
            close_pending: false,
            save_task: None,
            saving: false,
            #[cfg(feature = "agent")]
            agent: Default::default(),
            #[cfg(windows)]
            printing: false,
            caret_on: true,
            blink: None,
            subscriptions: vec![font_subscription, style_subscription, command_subscription],
            viewport_bounds: None,
            reveal_input: false,
            font_picker,
            format_state: None,
            font_document: None,
            styles_open: false,
            style_pick: None,
            style_catalog: Vec::new(),
            style_catalog_key: None,
            style_search,
            command_search,
            asset_task: None,
            asset_epoch: 0,
            gesture: None,
            advanced: advanced::Advanced::default(),
        }
    }
    fn palette(&self) -> Palette {
        if self.shell.dark {
            Palette::dark(Brand::Sheet)
        } else {
            Palette::light(Brand::Sheet)
        }
    }
    fn execute(&mut self, id: &str, window: &mut Window, cx: &mut Context<Self>) {
        match id {
            "file.recover_latest" => {
                if let Some(id) = self.canvas.reader.as_ref().and_then(|r| r.recovery_first()) {
                    self.engine_command(
                        "file.recover",
                        &serde_json::json!({"restore":id}),
                        window,
                        cx,
                    );
                }
                return;
            }
            "file.confirm_original" => {
                window.focus(&self.focus, cx);
                self.confirm_original(cx);
                return;
            }
            "file.cancel_replace" => {
                self.replace_confirm = None;
                window.focus(&self.focus, cx);
                cx.notify();
                return;
            }
            "file.save" => {
                self.save_document(false, cx);
                return;
            }
            "file.save_as" => {
                self.save_document(true, cx);
                return;
            }
            "file.new" => {
                self.begin_open(OpenSource::Blank, cx);
                return;
            }
            "file.discard" => {
                self.discard_pending(window, cx);
                return;
            }
            "file.keep" => {
                self.confirm_discard = false;
                self.pending_open = None;
                self.close_pending = false;
                window.focus(&self.focus, cx);
                cx.notify();
                return;
            }
            "edit.undo" | "edit.redo" | "select.all" => {
                self.engine_command(id, &serde_json::json!({}), window, cx);
                window.focus(&self.focus, cx);
                return;
            }
            "edit.copy" | "edit.cut" | "edit.paste" => {
                self.clipboard(id, window, cx);
                return;
            }
            "file.open" => {
                self.pick_open(cx);
                return;
            }
            "file.cancel_open" => {
                self.cancel_open(cx);
                return;
            }
            "view.next_page" => {
                self.page_step(1, cx);
                return;
            }
            "view.previous_page" => {
                self.page_step(-1, cx);
                return;
            }
            _ => {}
        }
        match self.shell.execute(id) {
            Ok(()) => {
                self.shell.notice = None;
                if id == "view.theme" {
                    gpui::component::Theme::change(
                        if self.shell.dark {
                            gpui::component::ThemeMode::Dark
                        } else {
                            gpui::component::ThemeMode::Light
                        },
                        Some(window),
                        cx,
                    );
                }
                if id == "file.new" {
                    self.file_page = false;
                    self.begin_open(OpenSource::Blank, cx);
                }
                window.set_window_title(&self.document_title());
            }
            Err(error) => self.shell.notice = Some(error),
        }
        cx.notify();
    }
    fn button(
        &self,
        id: &'static str,
        label: impl Into<SharedString>,
        command: &'static str,
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        let p = self.palette();
        let label = label.into();
        let accessible: SharedString = match command {
            "edit.undo" => self.shell.tr("撤销", "Undo").into(),
            "edit.redo" => self.shell.tr("重做", "Redo").into(),
            "view.theme" => self.shell.tr("切换外观", "Appearance").into(),
            _ => label.clone(),
        };
        let enabled = match command {
            "edit.undo" => self.canvas.reader.as_ref().is_some_and(|r| r.can_undo()),
            "edit.redo" => self.canvas.reader.as_ref().is_some_and(|r| r.can_redo()),
            "edit.copy" | "edit.cut" => self
                .canvas
                .reader
                .as_ref()
                .is_some_and(|r| !r.engine().sel.is_collapsed()),
            "file.save" | "file.save_as" => {
                !self.saving
                    && !self.canvas.loading
                    && self
                        .canvas
                        .reader
                        .as_ref()
                        .is_some_and(|r| !r.is_composing())
            }
            _ => true,
        };
        if !enabled {
            return joyoffice_ui::control(p, false, false)
                .id(id)
                .role(gpui::Role::Button)
                .aria_label(accessible.clone())
                .a11y_synthetic_children(|b| b.parent_node().set_disabled())
                .child(label);
        }
        joyoffice_ui::control(p, true, false)
            .id(id)
            .occlude()
            .role(gpui::Role::Button)
            .aria_label(accessible.clone())
            .key_context("JoyWordControl")
            .tab_stop(true)
            .focus(move |s| s.border_color(rgb(p.accent)))
            .cursor_pointer()
            .hover(move |s| s.bg(rgb(p.hover)))
            .on_click(cx.listener(move |this, _, window, cx| this.execute(command, window, cx)))
            .on_key_down(
                cx.listener(move |this, event: &gpui::KeyDownEvent, window, cx| {
                    if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                        this.execute(command, window, cx);
                        cx.stop_propagation();
                    }
                }),
            )
            .child(label)
    }
    fn unavailable(&self, text: &'static str) -> gpui::Stateful<gpui::Div> {
        joyoffice_ui::control(self.palette(), false, false)
            .id(SharedString::from(format!("disabled-{text}")))
            .role(gpui::Role::Button)
            .aria_label(text)
            .aria_description("Not available in this preview")
            .a11y_synthetic_children(|builder| builder.parent_node().set_disabled())
            .child(text)
    }
    fn header(&self, cx: &mut Context<Self>) -> gpui::Div {
        let p = self.palette();
        let tr = |zh, en| self.shell.tr(zh, en);
        div()
            .h(px(metrics::HEADER))
            .flex_shrink_0()
            .px_4()
            .flex()
            .items_center()
            .gap_2()
            .bg(rgb(p.header))
            .child(
                div()
                    .font_weight(gpui::FontWeight::BOLD)
                    .text_color(rgb(p.accent))
                    .child("JoyWord"),
            )
            .child(self.button("new-header", tr("新建", "New"), "file.new", cx))
            .child(self.button("open-header", tr("打开", "Open"), "file.open", cx))
            .child(self.button("save-header", tr("保存", "Save"), "file.save", cx))
            .child(self.button("undo-header", "↶", "edit.undo", cx))
            .child(self.button("redo-header", "↷", "edit.redo", cx))
            .child(div().w(px(1.)).h(px(18.)).bg(rgb(p.border)))
            .child(
                div()
                    .max_w(px(280.))
                    .truncate()
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .child(self.document_title()),
            )
            .child(div().flex_1())
            .child(
                joyoffice_ui::control(p, true, false)
                    .id("command-search")
                    .role(gpui::Role::Button)
                    .aria_label(tr("搜索命令", "Search commands"))
                    .tab_stop(true)
                    .w(px(236.))
                    .justify_start()
                    .bg(rgb(p.surface))
                    .text_color(rgb(p.muted))
                    .cursor_pointer()
                    .focus(move |s| s.border_color(rgb(p.accent)))
                    .child(tr("搜索命令   Ctrl+K", "Search commands   Ctrl+K"))
                    .on_click(cx.listener(|v, _, w, cx| v.open_command_search(w, cx)))
                    .on_key_down(cx.listener(|v, e: &gpui::KeyDownEvent, w, cx| {
                        if matches!(e.keystroke.key.as_str(), "enter" | "space") {
                            v.open_command_search(w, cx);
                            cx.stop_propagation();
                        }
                    })),
            )
            .child(self.button("language", tr("EN", "中文"), "view.language", cx))
            .child(self.button(
                "theme",
                if self.shell.dark { "☀" } else { "◐" },
                "view.theme",
                cx,
            ))
    }
    fn ribbon(&self, cx: &mut Context<Self>) -> gpui::Div {
        let p = self.palette();
        let tr = |zh, en| self.shell.tr(zh, en);
        let tabs = [
            (None, "文件", "File"),
            (Some(0), "开始", "Home"),
            (Some(1), "插入", "Insert"),
            (Some(2), "布局", "Layout"),
            (Some(3), "引用", "References"),
            (Some(8), "邮件", "Mailings"),
            (Some(4), "审阅", "Review"),
            (Some(5), "视图", "View"),
            (Some(9), "自动化", "Automate"),
            (Some(6), "表格", "Table"),
            (Some(7), "图片", "Picture"),
        ];
        let tab_row = div()
            .id("ribbon-tabs")
            .overflow_x_scroll()
            .flex()
            .flex_shrink_0()
            .h(px(metrics::TAB))
            .items_center()
            .px_3()
            .gap_1()
            .bg(rgb(p.chrome))
            .role(gpui::Role::TabList)
            .aria_label(tr("功能区", "Ribbon"))
            .children(
                tabs.into_iter()
                    .enumerate()
                    .map(|(position, (index, zh, en))| {
                        let label = tr(zh, en);
                        let active =
                            index.map_or(self.file_page, |i| !self.file_page && self.tab == i);
                        let activate = move |v: &mut JoyWord| {
                            v.file_page = index.is_none();
                            if let Some(index) = index {
                                v.tab = index;
                            }
                        };
                        div()
                            .id(("ribbon-tab", position))
                            .role(gpui::Role::Tab)
                            .aria_label(label)
                            .aria_selected(active)
                            .tab_stop(true)
                            .h_full()
                            .px_3()
                            .flex()
                            .items_center()
                            .cursor_pointer()
                            .border_b_2()
                            .border_color(rgb(if active { p.accent } else { p.chrome }))
                            .when(active, |el| {
                                el.font_weight(gpui::FontWeight::SEMIBOLD)
                                    .text_color(rgb(p.accent))
                            })
                            .focus(move |el| el.bg(rgb(p.hover)))
                            .child(label)
                            .on_click(cx.listener(move |v, _, _, cx| {
                                activate(v);
                                cx.notify();
                            }))
                            .on_key_down(cx.listener(move |v, e: &gpui::KeyDownEvent, _, cx| {
                                match e.keystroke.key.as_str() {
                                    "enter" | "space" => activate(v),
                                    _ => return,
                                }
                                cx.stop_propagation();
                                cx.notify();
                            }))
                    }),
            );
        let mut groups = div()
            .id("ribbon-controls")
            .flex()
            .items_center()
            .overflow_x_scroll()
            .h(px(88.0))
            .flex_shrink_0()
            .bg(rgb(p.surface))
            .border_b_1()
            .border_color(rgb(p.border));
        groups = match self.tab {
            0 => groups
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .justify_between()
                        .h(px(78.0))
                        .px_3()
                        .border_r_1()
                        .border_color(rgb(p.separator))
                        .child(
                            div()
                                .flex()
                                .gap_1()
                                .child(self.button(
                                    "paste-ribbon",
                                    tr("粘贴", "Paste"),
                                    "edit.paste",
                                    cx,
                                ))
                                .child(self.button("cut-ribbon", tr("剪切", "Cut"), "edit.cut", cx))
                                .child(self.button(
                                    "copy-ribbon",
                                    tr("复制", "Copy"),
                                    "edit.copy",
                                    cx,
                                )),
                        )
                        .child(
                            div()
                                .flex()
                                .gap_1()
                                .child(
                                    Button::new("format-brush")
                                        .ghost()
                                        .xsmall()
                                        .label("格式刷")
                                        .selected(self.canvas.reader.as_ref().is_some_and(|r| {
                                            r.engine().painter.as_ref().is_some_and(|p| !p.2)
                                        }))
                                        .disabled(!self.format_ready("edit.formatPainter"))
                                        .on_click(
                                            cx.listener(|v, _, w, cx| v.start_brush(false, w, cx)),
                                        ),
                                )
                                .child(
                                    Button::new("format-brush-sticky")
                                        .ghost()
                                        .xsmall()
                                        .label("连续刷")
                                        .selected(self.canvas.reader.as_ref().is_some_and(|r| {
                                            r.engine().painter.as_ref().is_some_and(|p| p.2)
                                        }))
                                        .disabled(!self.format_ready("edit.formatPainter"))
                                        .on_click(
                                            cx.listener(|v, _, w, cx| v.start_brush(true, w, cx)),
                                        ),
                                ),
                        )
                        .child(
                            div()
                                .text_size(px(11.0))
                                .text_color(rgb(p.muted))
                                .child(tr("剪贴板", "Clipboard")),
                        ),
                )
                .child(self.home_format_groups(cx))
                .child(self.search_ribbon(cx))
                .when(cfg!(feature = "agent"), |group| {
                    #[cfg(feature = "agent")]
                    return group.child(self.agent_controls(cx));
                    #[cfg(not(feature = "agent"))]
                    group
                }),
            1 => groups.child(
                div()
                    .flex()
                    .px_4()
                    .gap_2()
                    .child(self.table_grid(cx))
                    .child(
                        Button::new("insert-picture-ribbon")
                            .ghost()
                            .xsmall()
                            .label("图片…")
                            .disabled(!self.format_ready("insert.picture"))
                            .on_click(cx.listener(|v, _, w, cx| v.pick_picture(false, w, cx))),
                    )
                    .child(self.unavailable(tr("页码", "Page number")))
                    .child(self.unavailable(tr("符号", "Symbol"))),
            ),
            2 => groups.child(self.page_setup_ribbon(cx)),
            3 => groups.child(self.references_ribbon(cx)),
            4 => groups.child(self.review_ribbon(cx)),
            8 => groups.child(self.mailings_ribbon(cx)),
            9 => groups.child(self.developer_ribbon(cx)),
            6 => groups.child(self.table_tools(cx)),
            7 => groups.child(self.picture_tools(cx)),
            _ => groups.child(
                div()
                    .flex()
                    .px_4()
                    .gap_2()
                    .child(self.button("zoom-out-ribbon", "−", "view.zoom_out", cx))
                    .child(self.button("zoom-reset-ribbon", "100%", "view.zoom_reset", cx))
                    .child(self.button("zoom-in-ribbon", "+", "view.zoom_in", cx))
                    .child(self.button(
                        "theme-ribbon",
                        tr("切换外观", "Appearance"),
                        "view.theme",
                        cx,
                    )),
            ),
        };
        div()
            .flex()
            .flex_col()
            .flex_shrink_0()
            .child(tab_row)
            .child(groups)
    }
    fn workspace(&self, cx: &mut Context<Self>) -> gpui::Div {
        let p = self.palette();
        let tr = |zh, en| self.shell.tr(zh, en);
        if self.file_page {
            return div()
                .flex()
                .flex_1()
                .p_8()
                .gap_8()
                .bg(rgb(p.canvas))
                .child(
                    div()
                        .w(px(180.0))
                        .flex()
                        .flex_col()
                        .gap_3()
                        .child(
                            div()
                                .text_size(px(28.0))
                                .font_weight(gpui::FontWeight::SEMIBOLD)
                                .child(tr("文件", "File")),
                        )
                        .child(
                            Button::new("about")
                                .ghost()
                                .label(tr("关于 JoyWord", "About JoyWord"))
                                .on_click(cx.listener(|v, _, _, cx| {
                                    v.about = !v.about;
                                    cx.notify();
                                })),
                        )
                        .child(self.button(
                            "new-file",
                            tr("新建文档", "New document"),
                            "file.new",
                            cx,
                        ))
                        .child(
                            Button::new("export-file")
                                .ghost()
                                .label("导出副本")
                                .on_click(cx.listener(|v, _, w, cx| {
                                    v.file_page = false;
                                    v.advanced_action("exchange-export", w, cx);
                                })),
                        )
                        .child(
                            Button::new("export-epub")
                                .ghost()
                                .label(tr("导出 EPUB…", "Export EPUB…"))
                                .disabled(self.canvas.reader.is_none() || self.canvas.loading)
                                .on_click(cx.listener(|v, _, w, cx| {
                                    v.start_checked_export(export::Format::Epub, w, cx);
                                })),
                        )
                        .child(
                            Button::new("export-pdfa")
                                .ghost()
                                .label(tr("导出 PDF/A-2b…", "Export PDF/A-2b…"))
                                .disabled(self.canvas.reader.is_none() || self.canvas.loading)
                                .on_click(cx.listener(|v, _, w, cx| {
                                    v.start_checked_export(export::Format::Archive, w, cx);
                                })),
                        )
                        .when(cfg!(windows), |menu| {
                            #[cfg(windows)]
                            return menu.child(
                                Button::new("system-print")
                                    .ghost()
                                    .label(tr("打印…", "Print…"))
                                    .disabled(
                                        self.canvas.reader.is_none()
                                            || self.canvas.loading
                                            || self.printing,
                                    )
                                    .on_click(cx.listener(|v, _, w, cx| v.open_print(w, cx))),
                            );
                            #[cfg(not(windows))]
                            menu
                        })
                        .child(self.button(
                            "open-file",
                            tr("打开文档", "Open document"),
                            "file.open",
                            cx,
                        )),
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_4()
                        .child(
                            div()
                                .text_size(px(24.0))
                                .child(tr("从空白页开始", "Start with a blank page")),
                        )
                        .child(
                            joyoffice_ui::card(p)
                                .w(px(220.0))
                                .h(px(260.0))
                                .p_5()
                                .child(
                                    div()
                                        .h(px(170.0))
                                        .bg(rgb(0xffffff))
                                        .border_1()
                                        .border_color(rgb(p.border)),
                                )
                                .child(
                                    Button::new("new-card")
                                        .primary()
                                        .label(tr("空白文档", "Blank document"))
                                        .on_click(cx.listener(|this, _, window, cx| {
                                            this.execute("file.new", window, cx)
                                        })),
                                ),
                        )
                        .child(div().text_color(rgb(p.muted)).text_size(px(13.0)).child(tr(
                            "已支持正文编辑、中文输入、撤销重做和 DOCX 保存。",
                            "Text editing, native input, Undo/Redo and DOCX saving are available.",
                        )))
                        .when(!self.shell.recent_documents().is_empty(), |panel| {
                            panel.child(tr("最近文档", "Recent documents")).child(
                                div()
                                    .id("recent-documents")
                                    .max_h(px(240.0))
                                    .overflow_y_scroll()
                                    .flex()
                                    .flex_col()
                                    .children(
                                        self.shell.recent_documents().iter().enumerate().map(
                                            |(index, path)| {
                                                let path = path.clone();
                                                Button::new(("recent-document", index))
                                                    .ghost()
                                                    .label(path.display().to_string())
                                                    .disabled(self.saving || self.close_pending)
                                                    .on_click(cx.listener(move |this, _, _, cx| {
                                                        this.begin_open(
                                                            OpenSource::File(path.clone()),
                                                            cx,
                                                        );
                                                    }))
                                            },
                                        ),
                                    ),
                            )
                        }),
                );
        }
        self.document_workspace(cx)
    }
}

impl Render for JoyWord {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.prepare_canvas(window, cx);
        self.sync_format(window, cx);
        self.sync_advanced();
        window.set_window_title(&self.document_title());
        let p = self.palette();
        let mut content = div()
            .size_full()
            .flex()
            .flex_col()
            .bg(rgb(p.surface))
            .text_color(rgb(p.text))
            .font_family(if cfg!(target_os = "windows") {
                "Microsoft YaHei UI"
            } else {
                "sans-serif"
            })
            .text_size(px(13.0))
            .when(self.canvas.reader.is_none() || self.file_page, |el| {
                el.track_focus(&self.focus)
            })
            .key_context(if self.focus.is_focused(window) {
                "JoyWord JoyWordEditor"
            } else {
                "JoyWord"
            })
            .on_drop(cx.listener(|v, paths: &gpui::ExternalPaths, _, cx| {
                if let [path] = paths.paths() {
                    v.begin_open(OpenSource::File(path.clone()), cx);
                } else {
                    v.shell.notice = Some(
                        v.shell
                            .tr("请拖入一个本地文档", "Drop one local document")
                            .into(),
                    );
                    cx.notify();
                }
            }))
            .on_action(cx.listener(|v, _: &SearchCommands, w, cx| v.open_command_search(w, cx)))
            .on_action(cx.listener(|v, _: &FindDocument, w, cx| v.open_find(false, w, cx)))
            .on_action(cx.listener(|v, _: &ReplaceDocument, w, cx| v.open_find(true, w, cx)))
            .on_action(cx.listener(|v, _: &GoToPage, w, cx| v.open_go_to(w, cx)))
            .on_action(cx.listener(|v, _: &DocumentTab, w, cx| v.document_tab(false, w, cx)))
            .on_action(cx.listener(|v, _: &DocumentBackTab, w, cx| v.document_tab(true, w, cx)))
            .on_action(
                cx.listener(|this, _: &NewDocument, window, cx| {
                    this.execute("file.new", window, cx)
                }),
            )
            .on_action(cx.listener(|this, _: &OpenDocument, _, cx| this.pick_open(cx)))
            .on_action(cx.listener(|this, _: &SaveDocument, _, cx| this.save_document(false, cx)))
            .on_action(cx.listener(|this, _: &SaveAsDocument, _, cx| this.save_document(true, cx)))
            .on_action(cx.listener(|this, _: &Undo, window, cx| {
                this.engine_command("edit.undo", &serde_json::json!({}), window, cx)
            }))
            .on_action(cx.listener(|this, _: &Redo, window, cx| {
                this.engine_command("edit.redo", &serde_json::json!({}), window, cx)
            }))
            .on_action(cx.listener(|this, _: &SelectAll, window, cx| {
                this.engine_command("select.all", &serde_json::json!({}), window, cx)
            }))
            .on_action(
                cx.listener(|this, _: &Copy, window, cx| this.clipboard("edit.copy", window, cx)),
            )
            .on_action(
                cx.listener(|this, _: &Cut, window, cx| this.clipboard("edit.cut", window, cx)),
            )
            .on_action(
                cx.listener(|this, _: &Paste, window, cx| this.clipboard("edit.paste", window, cx)),
            )
            .on_action(cx.listener(|this, _: &Quit, window, cx| this.request_close(window, cx)))
            .on_key_down(
                cx.listener(|this, event, window, cx| this.document_key(event, window, cx)),
            )
            .on_action(
                cx.listener(|this, _: &ZoomIn, window, cx| {
                    this.execute("view.zoom_in", window, cx)
                }),
            )
            .on_action(cx.listener(|this, _: &ZoomOut, window, cx| {
                this.execute("view.zoom_out", window, cx)
            }))
            .on_action(cx.listener(|this, _: &ZoomReset, window, cx| {
                this.execute("view.zoom_reset", window, cx)
            }))
            .on_action(cx.listener(|this, _: &ToggleTheme, window, cx| {
                this.execute("view.theme", window, cx)
            }))
            .on_action(cx.listener(|this, _: &ToggleLanguage, window, cx| {
                this.execute("view.language", window, cx)
            }))
            .child(self.header(cx))
            .child(self.ribbon(cx));
        if self
            .canvas
            .reader
            .as_ref()
            .is_some_and(|r| r.engine().painter.is_some())
        {
            content = content.child(
                div()
                    .px_4()
                    .py_1()
                    .bg(rgb(p.selected))
                    .child("格式刷已开启：拖选目标文字或单击目标段落；Esc 结束。"),
            );
        }
        if self
            .canvas
            .reader
            .as_ref()
            .and_then(|r| r.recovery_first())
            .is_some()
        {
            content = content.child(
                div()
                    .p_2()
                    .bg(rgb(p.selected))
                    .flex()
                    .gap_3()
                    .items_center()
                    .child(self.shell.tr(
                        "发现未保存的恢复副本。恢复后请另存为 DOCX。",
                        "An unsaved recovery copy is available. Save As after restoring.",
                    ))
                    .child(self.button(
                        "recover-latest",
                        self.shell.tr("恢复最新副本", "Recover latest"),
                        "file.recover_latest",
                        cx,
                    )),
            );
        }
        if let Some(path) = self.legacy_recovery.as_ref() {
            let path = path.clone();
            content = content.child(
                div()
                    .p_2()
                    .bg(rgb(p.selected))
                    .flex()
                    .gap_3()
                    .items_center()
                    .child(self.shell.tr(
                        "发现旧版恢复日志。恢复后请另存为 DOCX。",
                        "A legacy recovery journal was found. Save As after restoring.",
                    ))
                    .child(
                        Button::new("recover-legacy")
                            .label(self.shell.tr("恢复旧版副本", "Recover legacy copy"))
                            .disabled(self.saving || self.canvas.loading || self.close_pending)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.begin_open(OpenSource::LegacyRecovery(path.clone()), cx);
                            })),
                    ),
            );
        }
        if self.replace_confirm.is_some() {
            content=content.child(div().p_3().bg(rgb(p.selected)).flex().flex_col().gap_2()
                .child(self.shell.tr("替换原文件：当前组件无法验证文件所有者、ACL 与全部扩展属性。建议另存副本。","Replace original: ownership, ACLs and all extended attributes cannot be verified. Save a copy is recommended."))
                .child(div().flex().gap_3()
                    .child(self.button("metadata-save-copy",self.shell.tr("另存副本","Save a copy"),"file.save_as",cx))
                    .child(self.button("metadata-confirm",self.shell.tr("确认替换原文件","Confirm replace original"),"file.confirm_original",cx))
                    .child(self.button("metadata-cancel",self.shell.tr("取消","Cancel"),"file.cancel_replace",cx))));
        }
        if self.confirm_discard {
            content = content.child(
                div()
                    .p_3()
                    .bg(rgb(p.selected))
                    .flex()
                    .items_center()
                    .gap_3()
                    .child(self.shell.tr(
                        "文档有未保存修改。请保存、放弃或取消。",
                        "Unsaved changes. Save, discard or cancel.",
                    ))
                    .child(self.button(
                        "confirm-save",
                        self.shell.tr("保存", "Save"),
                        "file.save",
                        cx,
                    ))
                    .child(self.button(
                        "confirm-discard",
                        self.shell.tr("放弃修改", "Discard"),
                        "file.discard",
                        cx,
                    ))
                    .child(self.button(
                        "confirm-keep",
                        self.shell.tr("取消", "Cancel"),
                        "file.keep",
                        cx,
                    )),
            );
        }
        if self.about {
            content = content.child(
                div()
                    .p_4()
                    .bg(rgb(p.selected))
                    .text_color(rgb(p.accent))
                    .child("JoyWord 0.1.1 · R5 DOCX editor · GPUI Kit 0.8.1 · JoyOffice shared UI"),
            );
        }
        if let Some(note) = &self.shell.notice {
            content = content.child(div().p_3().bg(rgb(p.selected)).child(note.clone()));
        }
        content
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_h_0()
                    .child(self.workspace(cx))
                    .when(self.styles_open, |s| s.child(self.styles_panel(cx)))
                    .when(self.advanced.pane.is_some(), |s| {
                        s.child(self.advanced_panel(cx))
                    }),
            )
            .child(
                div()
                    .flex()
                    .id("document-status")
                    .role(gpui::Role::Status)
                    .h(px(25.0))
                    .flex_shrink_0()
                    .px_4()
                    .items_center()
                    .justify_between()
                    .bg(rgb(p.surface))
                    .border_t_1()
                    .border_color(rgb(p.border))
                    .text_color(rgb(p.muted))
                    .child(self.document_status())
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(self.button("zoom-out", "−", "view.zoom_out", cx))
                            .child(self.button(
                                "zoom-reset",
                                format!("{}%", self.shell.zoom),
                                "view.zoom_reset",
                                cx,
                            ))
                            .child(self.button("zoom-in", "+", "view.zoom_in", cx)),
                    ),
            )
    }
}

fn bind_document_keys(cx: &mut App) {
    // Kit Root handles Tab as focus traversal before raw key-down listeners run.
    // Match the editor's exact focus only; controls and modal inputs keep native Tab.
    cx.bind_keys([
        KeyBinding::new("tab", DocumentTab, Some("JoyWordEditor")),
        KeyBinding::new("shift-tab", DocumentBackTab, Some("JoyWordEditor")),
        KeyBinding::new("ctrl-f", FindDocument, Some("JoyWordEditor")),
        KeyBinding::new("cmd-f", FindDocument, Some("JoyWordEditor")),
        KeyBinding::new("ctrl-h", ReplaceDocument, Some("JoyWordEditor")),
        KeyBinding::new("f5", GoToPage, Some("JoyWordEditor")),
    ]);
}

pub fn run() {
    let application = gpui_kit::application();
    #[cfg(target_os = "macos")]
    let open_requests = {
        let (send, receive) = async_channel::bounded(16);
        application.on_open_urls(move |urls| {
            // This editor has one document window. Use the first supported file in a batch.
            let path = urls.iter().find_map(|value| {
                let path = url::Url::parse(value).ok()?.to_file_path().ok()?;
                path.extension()
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("docx"))
                    .then_some(path)
            });
            if let Some(path) = path
                && send.try_send(path).is_err()
            {
                eprintln!("JoyWord could not queue a macOS document open request");
            }
        });
        receive
    };
    application
        .with_assets(gpui_kit::assets::AllAssets)
        .run(move |cx: &mut App| {
            gpui_kit::init(cx);
            bind_document_keys(cx);

            cx.bind_keys([
                KeyBinding::new("ctrl-s", SaveDocument, Some("JoyWord")),
                KeyBinding::new("ctrl-shift-s", SaveAsDocument, Some("JoyWord")),
                KeyBinding::new("ctrl-z", Undo, Some("JoyWord")),
                KeyBinding::new("ctrl-y", Redo, Some("JoyWord")),
                KeyBinding::new("ctrl-shift-z", Redo, Some("JoyWord")),
                KeyBinding::new("ctrl-a", SelectAll, Some("JoyWord")),
                KeyBinding::new("ctrl-c", Copy, Some("JoyWord")),
                KeyBinding::new("ctrl-x", Cut, Some("JoyWord")),
                KeyBinding::new("ctrl-v", Paste, Some("JoyWord")),
                KeyBinding::new("cmd-s", SaveDocument, Some("JoyWord")),
                KeyBinding::new("cmd-shift-s", SaveAsDocument, Some("JoyWord")),
                KeyBinding::new("cmd-z", Undo, Some("JoyWord")),
                KeyBinding::new("cmd-shift-z", Redo, Some("JoyWord")),
                KeyBinding::new("cmd-a", SelectAll, Some("JoyWord")),
                KeyBinding::new("cmd-c", Copy, Some("JoyWord")),
                KeyBinding::new("cmd-x", Cut, Some("JoyWord")),
                KeyBinding::new("cmd-v", Paste, Some("JoyWord")),
                KeyBinding::new("ctrl-k", SearchCommands, Some("JoyWord")),
                KeyBinding::new("cmd-k", SearchCommands, Some("JoyWord")),
                KeyBinding::new("ctrl-n", NewDocument, Some("JoyWord")),
                KeyBinding::new("ctrl-o", OpenDocument, Some("JoyWord")),
                KeyBinding::new("cmd-o", OpenDocument, Some("JoyWord")),
                KeyBinding::new("cmd-n", NewDocument, Some("JoyWord")),
                KeyBinding::new("ctrl-=", ZoomIn, Some("JoyWord")),
                KeyBinding::new("ctrl--", ZoomOut, Some("JoyWord")),
                KeyBinding::new("ctrl-0", ZoomReset, Some("JoyWord")),
            ]);
            cx.set_menus(vec![
                Menu {
                    name: "File".into(),
                    disabled: false,
                    items: vec![
                        MenuItem::action("New", NewDocument),
                        MenuItem::action("Open DOCX", OpenDocument),
                        MenuItem::action("Save", SaveDocument),
                        MenuItem::action("Save As", SaveAsDocument),
                        MenuItem::action("Quit", Quit),
                    ],
                },
                Menu {
                    name: "View".into(),
                    disabled: false,
                    items: vec![
                        MenuItem::action("Zoom In", ZoomIn),
                        MenuItem::action("Zoom Out", ZoomOut),
                        MenuItem::action("Actual Size", ZoomReset),
                        MenuItem::action("Appearance", ToggleTheme),
                        MenuItem::action("Language", ToggleLanguage),
                    ],
                },
            ]);
            let bounds = Bounds::centered(None, size(px(1440.0), px(900.0)), cx);
            let result = gpui_kit::open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    window_min_size: Some(size(px(700.0), px(500.0))),
                    titlebar: Some(gpui::TitlebarOptions {
                        title: Some("JoyWord".into()),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                cx,
                |window, cx| {
                    let view = cx.new(|cx| JoyWord::new(window, cx));
                    let close_view = view.downgrade();
                    window.on_window_should_close(cx, move |window, cx| {
                        let _ = close_view.update(cx, |this, cx| this.request_close(window, cx));
                        false
                    });
                    view.update(cx, |this, cx| {
                        this.start_blink(cx);
                        cx.spawn(async |view, cx| {
                            let path = cx
                                .background_executor()
                                .spawn(async { joyword_shell::legacy_recovery_directory() })
                                .await;
                            if let Some(path) = path {
                                let _ = view.update(cx, |this, cx| {
                                    this.legacy_recovery = Some(path);
                                    cx.notify();
                                });
                            }
                        })
                        .detach();
                        let focus = this.focus.clone();
                        this.subscriptions.push(
                            cx.on_blur(&focus, window, |v, window, cx| {
                                v.cancel_preedit(window, cx)
                            }),
                        );
                        this.subscriptions.push(cx.observe_window_activation(
                            window,
                            |v, window, cx| {
                                if !window.is_window_active() {
                                    v.cancel_preedit(window, cx);
                                }
                            },
                        ));
                    });
                    let (port, source) = startup_options(std::env::args_os().skip(1));
                    if let Some(port) = port {
                        control::install(port, &view, window, cx);
                    }
                    let source = source.map(OpenSource::File).unwrap_or(OpenSource::Welcome);
                    view.update(cx, |this, cx| this.begin_open(source, cx));
                    #[cfg(target_os = "macos")]
                    listen_for_system_open(&view, window, cx, open_requests);
                    view
                },
            );
            if let Err(error) = result {
                eprintln!("JoyWord could not open a window: {error}");
                cx.quit();
            }
            cx.activate(true);
        });
}

#[cfg(target_os = "macos")]
fn listen_for_system_open(
    view: &gpui::Entity<JoyWord>,
    window: &Window,
    cx: &App,
    requests: async_channel::Receiver<std::path::PathBuf>,
) {
    let handle = window.window_handle();
    let view = view.downgrade();
    cx.spawn(async move |cx| {
        while let Ok(path) = requests.recv().await {
            let opened = cx.update(|cx| {
                handle.update(cx, |_, window, cx| {
                    window.activate_window();
                    cx.activate(true);
                    view.update(cx, |this, cx| {
                        if this.saving || this.close_pending {
                            this.shell.notice = Some(
                                this.shell.tr(
                                    "请先完成保存或关闭，再重新打开文档。",
                                    "Finish saving or closing before retrying the open request.",
                                ).into(),
                            );
                            cx.notify();
                        } else if this
                            .canvas
                            .reader
                            .as_ref()
                            .and_then(|reader| reader.source_path())
                            != Some(path.as_path())
                        {
                            this.begin_open(OpenSource::File(path), cx);
                        }
                    })
                })
            });
            if !matches!(opened, Ok(Ok(()))) {
                break;
            }
        }
    })
    .detach();
}

/// Match the JoyOffice launch convention, including an optional `--` before
/// a file name that starts with a hyphen.
fn startup_options(
    args: impl IntoIterator<Item = std::ffi::OsString>,
) -> (Option<u16>, Option<std::path::PathBuf>) {
    let mut options = true;
    let mut port = None;
    let mut path = None;
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        if options && arg == "--" {
            options = false;
        } else if options && arg == "--control" {
            port = args
                .next()
                .and_then(|v| v.to_str()?.parse().ok())
                .filter(|p| *p != 0);
        } else if path.is_none() && (!options || !arg.to_string_lossy().starts_with('-')) {
            path = Some(arg.into());
        }
    }
    (port, path)
}
