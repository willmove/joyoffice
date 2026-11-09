mod advanced;
#[cfg(feature = "agent")]
mod agent;
#[cfg(feature = "agent")]
mod agent_port;
mod clipboard;
mod command_search;
mod control;
mod document_view;
mod export;
mod file_page;
mod formatting;
mod forms;
mod gestures;
mod header_footer;
mod input;
mod insert;
mod layout_ribbon;
mod objects;
mod page_index;
mod page_setup;
mod perf;
#[cfg(windows)]
mod printing;
mod ribbon;
mod save;
mod search;
mod styles;
mod text_accessibility;
mod view_ribbon;
use document_view::{DocumentCanvas, DocumentChange, OpenSource};
use file_page::FileSection;
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
    file_section: FileSection,
    file_scroll: ScrollHandle,
    canvas: DocumentCanvas,
    input: Option<gpui::Entity<input::InputBridge>>,
    page_bounds: std::collections::BTreeMap<usize, Bounds<gpui::Pixels>>,
    dragging: bool,
    confirm_discard: bool,
    replace_confirm: Option<(u64, u64)>,
    pending_document: Option<DocumentChange>,
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
    layout: layout_ribbon::LayoutControls,
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
        let layout = layout_ribbon::LayoutControls::new(window, cx);
        let mut subscriptions = vec![font_subscription, style_subscription, command_subscription];
        for (index, input) in layout.inputs.iter().enumerate() {
            subscriptions.push(cx.subscribe_in(input, window, move |v, _, event, w, cx| {
                v.layout_input_event(index, event, w, cx);
            }));
        }
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
            file_section: FileSection::default(),
            file_scroll: ScrollHandle::new(),
            canvas: DocumentCanvas::default(),
            input: None,
            page_bounds: Default::default(),
            dragging: false,
            confirm_discard: false,
            replace_confirm: None,
            pending_document: None,
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
            subscriptions,
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
            layout,
        }
    }
    fn palette(&self) -> Palette {
        if self.shell.dark {
            Palette::dark(Brand::Word)
        } else {
            Palette::light(Brand::Word)
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
                self.pending_document = None;
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
        if matches!(id, "view.zoom_in" | "view.zoom_out" | "view.zoom_reset") {
            let value = match id {
                "view.zoom_in" => self.shell.zoom.saturating_add(10).min(500),
                "view.zoom_out" => self.shell.zoom.saturating_sub(10).max(10),
                _ => 100,
            };
            self.run_format("view.zoom", &serde_json::json!({"value":value}), window, cx);
            return;
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
            "view.zoom_in" | "view.zoom_out" | "view.zoom_reset" => self.format_ready("view.zoom"),
            "edit.undo" => self.canvas.reader.as_ref().is_some_and(|r| r.can_undo()),
            "edit.redo" => self.canvas.reader.as_ref().is_some_and(|r| r.can_redo()),
            "edit.copy" | "edit.cut" => self
                .canvas
                .reader
                .as_ref()
                .is_some_and(|r| !r.engine().sel.is_collapsed()),
            "file.save" | "file.save_as" => self.save_available(),
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
                        let active = index == Some(self.tab);
                        let activate =
                            move |v: &mut JoyWord, w: &mut Window, cx: &mut Context<Self>| {
                                if let Some(index) = index {
                                    v.tab = index;
                                    cx.notify();
                                } else {
                                    v.open_file_page(w, cx);
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
                            .on_click(cx.listener(move |v, _, w, cx| {
                                activate(v, w, cx);
                            }))
                            .on_key_down(cx.listener(move |v, e: &gpui::KeyDownEvent, w, cx| {
                                match e.keystroke.key.as_str() {
                                    "enter" | "space" => activate(v, w, cx),
                                    _ => return,
                                }
                                cx.stop_propagation();
                            }))
                    }),
            );
        let mut groups = div()
            .id("ribbon-controls")
            .flex()
            .items_center()
            .overflow_x_scroll()
            .h(px(if matches!(self.tab, 1 | 2 | 5) {
                76.0
            } else {
                88.0
            }))
            .flex_shrink_0()
            .bg(rgb(p.surface))
            .border_b_1()
            .border_color(rgb(p.border))
            .when(matches!(self.tab, 1 | 2 | 5), |group| {
                group.bg(rgb(p.chrome))
            });
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
            1 => groups.child(self.insert_ribbon(cx)),
            2 => groups.child(self.layout_ribbon(cx)),
            3 => groups.child(self.references_ribbon(cx)),
            4 => groups.child(self.review_ribbon(cx)),
            5 => groups.child(self.view_ribbon(cx)),
            8 => groups.child(self.mailings_ribbon(cx)),
            9 => groups.child(self.developer_ribbon(cx)),
            6 => groups.child(self.table_tools(cx)),
            7 => groups.child(self.picture_tools(cx)),
            _ => groups,
        };
        div()
            .flex()
            .flex_col()
            .flex_shrink_0()
            .child(tab_row)
            .child(groups)
    }
}

impl Render for JoyWord {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.sync_view_zoom();
        self.prepare_canvas(window, cx);
        self.sync_format(window, cx);
        if self.tab == 2 {
            self.sync_layout_controls(window, cx);
        }
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
            .key_context(if !self.file_page && self.focus.is_focused(window) {
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
            .on_key_down(cx.listener(|this, event: &gpui::KeyDownEvent, window, cx| {
                if this.file_page && event.keystroke.key == "escape" {
                    if this.confirm_discard {
                        this.execute("file.keep", window, cx);
                    } else if this.replace_confirm.is_some() {
                        this.execute("file.cancel_replace", window, cx);
                    } else {
                        this.return_to_document(window, cx);
                    }
                    cx.stop_propagation();
                } else {
                    this.document_key(event, window, cx);
                }
            }))
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
            .when(!self.file_page && !self.view_focus(), |el| {
                el.child(self.header(cx)).child(self.ribbon(cx))
            })
            .when(self.view_focus(), |el| el.child(self.focus_bar(cx)));
        if !self.file_page
            && self
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
        if !self.file_page
            && self
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
        if !self.file_page
            && let Some(path) = self.legacy_recovery.as_ref()
        {
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
        if self.file_page && self.canvas.loading {
            content = content.child(
                div()
                    .px_4()
                    .py_1()
                    .flex()
                    .gap_3()
                    .items_center()
                    .bg(rgb(p.selected))
                    .child(self.document_status())
                    .child(self.button(
                        "file-cancel-open",
                        self.shell.tr("取消", "Cancel"),
                        "file.cancel_open",
                        cx,
                    )),
            );
        }
        if let Some(note) = &self.shell.notice {
            content = content.child(div().p_3().bg(rgb(p.selected)).child(note.clone()));
        }
        if self.file_page {
            return content.child(self.render_file_page(cx));
        }
        content
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_h_0()
                    .when(self.view_navigation(), |s| {
                        s.child(self.navigation_panel(cx))
                    })
                    .child(self.document_workspace(cx))
                    .when(self.styles_open && !self.view_focus(), |s| {
                        s.child(self.styles_panel(cx))
                    })
                    .when(self.advanced.pane.is_some() && !self.view_focus(), |s| {
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
                    app_id: Some("joyword".into()),
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
