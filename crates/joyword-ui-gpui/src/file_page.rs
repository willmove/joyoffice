use super::*;
use gpui::component::scroll::ScrollableElement;
use gpui::component::tooltip::Tooltip;
use gpui::{Div, ElementId, KeyDownEvent, RenderImage, Role, Stateful, img, point};
use joyoffice_ui::{IconName, icon};
use std::{
    path::Path,
    rc::Rc,
    sync::{Arc, OnceLock},
};

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub(super) enum FileSection {
    #[default]
    Home,
    New,
    Open,
    Info,
    Recovery,
    #[cfg(windows)]
    Print,
    Export,
    Options,
    About,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum SidebarAction {
    Back,
    Section(FileSection),
    Save,
    SaveAs,
    Close,
}

struct Template {
    labels: (&'static str, &'static str),
    document: Option<&'static [u8]>,
    preview_png: Option<&'static [u8]>,
    preview: OnceLock<Option<Arc<RenderImage>>>,
}

impl Template {
    const fn new(
        labels: (&'static str, &'static str),
        document: Option<&'static [u8]>,
        preview_png: Option<&'static [u8]>,
    ) -> Self {
        Self {
            labels,
            document,
            preview_png,
            preview: OnceLock::new(),
        }
    }

    fn preview(&self) -> Option<Arc<RenderImage>> {
        self.preview
            .get_or_init(|| {
                let mut pixels = image::load_from_memory(self.preview_png?)
                    .ok()?
                    .into_rgba8();
                for pixel in pixels.pixels_mut() {
                    pixel.0.swap(0, 2);
                }
                Some(Arc::new(RenderImage::new(vec![image::Frame::new(pixels)])))
            })
            .clone()
    }
}

static TEMPLATES: [Template; 5] = [
    Template::new(("空白文档", "Blank document"), None, None),
    Template::new(
        ("工作室手册（示例）", "Studio Handbook"),
        Some(include_bytes!("../../../assets/samples/handbook.docx")),
        Some(include_bytes!(
            "../../../assets/samples/previews/handbook.png"
        )),
    ),
    Template::new(
        ("信函", "Business letter"),
        Some(include_bytes!("../../../assets/samples/letter.docx")),
        Some(include_bytes!(
            "../../../assets/samples/previews/letter.png"
        )),
    ),
    Template::new(
        ("简历", "Résumé"),
        Some(include_bytes!("../../../assets/samples/resume.docx")),
        Some(include_bytes!(
            "../../../assets/samples/previews/resume.png"
        )),
    ),
    Template::new(
        ("JoyWord 欢迎文档", "Welcome to JoyWord"),
        Some(joyword_shell::reader::WELCOME),
        Some(include_bytes!(
            "../../../assets/samples/previews/welcome.png"
        )),
    ),
];

impl JoyWord {
    pub(super) fn open_file_page(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.cancel_preedit(window, cx);
        self.file_page = true;
        self.select_file_section(FileSection::Home, cx);
        window.focus(&self.focus, cx);
    }

    pub(super) fn return_to_document(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.canvas.reader.is_some() {
            self.file_page = false;
            window.focus(&self.focus, cx);
            cx.notify();
        }
    }

    pub(super) fn select_file_section(&mut self, section: FileSection, cx: &mut Context<Self>) {
        self.file_section = section;
        self.file_scroll.set_offset(point(px(0.0), px(0.0)));
        cx.notify();
    }

    fn file_change_available(&self) -> bool {
        !self.saving
            && !self.canvas.loading
            && !self.confirm_discard
            && self.replace_confirm.is_none()
    }

    pub(super) fn render_file_page(&self, cx: &mut Context<Self>) -> Div {
        let p = self.palette();
        let tr = |zh, en| self.shell.tr(zh, en);
        let more_templates = tr("更多模板", "More templates");
        let (title, body) = match self.file_section {
            FileSection::Home => (
                tr("欢迎使用 JoyWord", "Welcome to JoyWord"),
                div()
                    .flex()
                    .flex_col()
                    .gap_5()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(self.template_gallery(&TEMPLATES[..4], cx))
                            .child(
                                file_control(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap_1()
                                        .h(px(28.0))
                                        .rounded(px(metrics::RADIUS))
                                        .text_color(rgb(p.accent))
                                        .child(more_templates)
                                        .child(icon(IconName::ArrowRight, 14.0, p.accent)),
                                    "file-more-templates",
                                    more_templates,
                                    true,
                                    p.accent,
                                    cx,
                                    |v, _, cx| v.select_file_section(FileSection::New, cx),
                                )
                                .self_start()
                                .hover(move |s| s.bg(rgb(p.hover))),
                            ),
                    )
                    .child(self.recent_files(cx)),
            ),
            FileSection::New => (tr("新建", "New"), self.template_gallery(&TEMPLATES, cx)),
            FileSection::Open => (
                tr("打开", "Open"),
                div()
                    .flex()
                    .flex_col()
                    .gap_6()
                    .child(
                        self.file_row(
                            "open-file",
                            tr("浏览本地文件…", "Browse local files…"),
                            IconName::FolderOpen,
                            self.file_change_available(),
                            cx,
                            |v, w, cx| {
                                w.focus(&v.focus, cx);
                                v.pick_open(cx);
                            },
                        )
                        .w_auto()
                        .self_start(),
                    )
                    .child(self.recent_files(cx)),
            ),
            FileSection::Info => (tr("信息", "Info"), self.file_info()),
            FileSection::Recovery => (tr("恢复", "Recover"), self.file_recovery(cx)),
            #[cfg(windows)]
            FileSection::Print => (tr("打印", "Print"), self.file_print(cx)),
            FileSection::Export => (tr("导出", "Export"), self.file_export(cx)),
            FileSection::Options => (tr("选项", "Options"), self.file_options(cx)),
            FileSection::About => (
                tr("关于 JoyWord", "About JoyWord"),
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(concat!("JoyWord ", env!("CARGO_PKG_VERSION")))
                    .child(
                        div()
                            .text_color(rgb(p.muted))
                            .child(tr("JoyOffice 的文字编辑器", "The JoyOffice word processor")),
                    ),
            ),
        };
        div()
            .flex()
            .flex_1()
            .min_h_0()
            .bg(rgb(p.canvas))
            .child(self.file_sidebar(cx))
            .child(
                div()
                    .id("file-page-content")
                    .relative()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_h_0()
                    .min_w_0()
                    .overflow_y_scroll()
                    .track_scroll(&self.file_scroll)
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .flex_shrink_0()
                            .gap_4()
                            .px(px(40.0))
                            .pt(px(30.0))
                            .pb_8()
                            .child(div().text_size(px(26.0)).line_height(px(36.0)).child(title))
                            .child(body),
                    )
                    .vertical_scrollbar(&self.file_scroll),
            )
    }

    fn file_sidebar(&self, cx: &mut Context<Self>) -> Div {
        use FileSection::*;
        use SidebarAction::{Back, Close, Save, SaveAs, Section};

        let p = self.palette();
        let tr = |zh, en| self.shell.tr(zh, en);
        let entries = [
            ("file-home", "开始", "Home", IconName::House, Section(Home)),
            ("file-new", "新建", "New", IconName::FilePlus, Section(New)),
            (
                "file-open",
                "打开",
                "Open",
                IconName::FolderOpen,
                Section(Open),
            ),
            ("file-info", "信息", "Info", IconName::Info, Section(Info)),
            (
                "file-recovery",
                "恢复",
                "Recover",
                IconName::RotateCcw,
                Section(Recovery),
            ),
            ("file-save", "保存", "Save", IconName::Save, Save),
            (
                "file-save-as",
                "另存为",
                "Save As",
                IconName::SaveAll,
                SaveAs,
            ),
            #[cfg(windows)]
            (
                "file-print",
                "打印",
                "Print",
                IconName::Printer,
                Section(Print),
            ),
            (
                "file-export",
                "导出",
                "Export",
                IconName::FileOutput,
                Section(Export),
            ),
            ("file-close", "关闭", "Close", IconName::X, Close),
            (
                "file-options",
                "选项",
                "Options",
                IconName::Settings,
                Section(Options),
            ),
        ];
        div()
            .w(px(200.0))
            .flex_shrink_0()
            .flex()
            .flex_col()
            .bg(rgb(if self.shell.dark { p.header } else { p.accent }))
            .child(
                div()
                    .flex()
                    .items_center()
                    .h(px(62.0))
                    .flex_shrink_0()
                    .px_3()
                    .child(
                        self.sidebar_item(
                            "file-back",
                            tr("返回文档", "Back to document"),
                            IconName::ArrowLeft,
                            Back,
                            cx,
                        )
                        .w(px(36.0))
                        .px_0()
                        .justify_center()
                        .rounded(px(metrics::RADIUS)),
                    ),
            )
            .child(
                div()
                    .id("file-page-navigation")
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .children(entries.into_iter().map(|(id, zh, en, icon, action)| {
                        self.sidebar_item(id, tr(zh, en), icon, action, cx)
                    })),
            )
            .child(div().flex_shrink_0().pb_2().child(self.sidebar_item(
                "file-about",
                tr("关于", "About"),
                IconName::Info,
                Section(About),
                cx,
            )))
    }

    fn sidebar_item(
        &self,
        id: &'static str,
        label: &'static str,
        symbol: IconName,
        action: SidebarAction,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let p = self.palette();
        let foreground = if self.shell.dark {
            p.accent
        } else {
            p.on_accent
        };
        let selected = action == SidebarAction::Section(self.file_section);
        let enabled = match action {
            SidebarAction::Back => self.canvas.reader.is_some(),
            SidebarAction::Save | SidebarAction::SaveAs => self.save_available(),
            SidebarAction::Close => self.canvas.reader.is_some() && self.file_change_available(),
            SidebarAction::Section(_) => true,
        };
        let highlight = if selected && self.shell.dark {
            rgb(p.selected)
        } else {
            rgb(foreground).alpha(if selected { 0.15 } else { 0.09 })
        };
        let el = div()
            .flex()
            .items_center()
            .flex_shrink_0()
            .gap_3()
            .h(px(42.0))
            .px(px(22.0))
            .text_size(px(14.0))
            .text_color(rgb(foreground))
            .when(selected, |el| {
                el.bg(highlight).font_weight(gpui::FontWeight::SEMIBOLD)
            })
            .child(icon(symbol, metrics::ICON, foreground))
            .when(action != SidebarAction::Back, |el| el.child(label))
            .when(!enabled, |el| el.opacity(0.55));
        file_control(
            el,
            id,
            label,
            enabled,
            foreground,
            cx,
            move |v, w, cx| match action {
                SidebarAction::Back => v.return_to_document(w, cx),
                SidebarAction::Section(section) => v.select_file_section(section, cx),
                SidebarAction::Save | SidebarAction::SaveAs => {
                    v.save_document(action == SidebarAction::SaveAs, cx);
                }
                SidebarAction::Close => {
                    w.focus(&v.focus, cx);
                    v.begin_document_change(DocumentChange::Close, cx);
                }
            },
        )
        .aria_selected(selected)
        .when(enabled, |el| el.hover(move |s| s.bg(highlight)))
    }

    fn template_gallery(&self, templates: &'static [Template], cx: &mut Context<Self>) -> Div {
        let p = self.palette();
        let enabled = self.file_change_available();
        div()
            .flex()
            .flex_wrap()
            .gap_x(px(16.0))
            .gap_y(px(12.0))
            .children(templates.iter().enumerate().map(|(index, template)| {
                let label = self.shell.tr(template.labels.0, template.labels.1);
                let el = div()
                    .w(px(152.0))
                    .flex_shrink_0()
                    .flex()
                    .flex_col()
                    .rounded(px(metrics::RADIUS))
                    .text_size(px(13.0))
                    .text_color(rgb(if enabled { p.text } else { p.disabled }))
                    .child(
                        joyoffice_ui::card(p)
                            .rounded(px(metrics::RADIUS))
                            .h(px(194.0))
                            .bg(rgb(Palette::light(Brand::Word).surface))
                            .overflow_hidden()
                            .children(template.preview().map(|preview| {
                                img(preview)
                                    .size_full()
                                    .object_fit(gpui::ObjectFit::Contain)
                            })),
                    )
                    .child(div().line_height(px(18.0)).truncate().child(label));
                file_control(
                    el,
                    ("file-template", index),
                    label,
                    enabled,
                    p.accent,
                    cx,
                    move |v, w, cx| {
                        w.focus(&v.focus, cx);
                        let source = match template.document {
                            Some(bytes) => OpenSource::Template {
                                name: format!("{label}.docx"),
                                bytes,
                            },
                            None => OpenSource::Blank,
                        };
                        v.begin_open(source, cx);
                    },
                )
                .when(enabled, |el| el.hover(move |s| s.bg(rgb(p.hover))))
            }))
    }

    fn recent_files(&self, cx: &mut Context<Self>) -> Div {
        let p = self.palette();
        let paths = self.shell.recent_documents();
        let enabled = self.file_change_available();
        div()
            .min_w_0()
            .flex()
            .flex_col()
            .gap_2()
            .child(
                div()
                    .text_size(px(17.0))
                    .child(self.shell.tr("最近", "Recent")),
            )
            .when(paths.is_empty(), |el| {
                el.child(div().text_color(rgb(p.muted)).child(self.shell.tr(
                    "最近打开的文档会显示在这里。",
                    "Recently opened documents will appear here.",
                )))
            })
            .child(
                div()
                    .flex()
                    .flex_col()
                    .children(paths.iter().enumerate().map(|(index, path)| {
                        let path = path.clone();
                        let location = display_path(&path);
                        let name = path.file_name().map_or_else(
                            || location.clone(),
                            |name| name.to_string_lossy().into_owned(),
                        );
                        let description = format!("{name}\n{location}");
                        let el = div()
                            .min_w_0()
                            .h(px(44.0))
                            .px_2()
                            .rounded(px(metrics::RADIUS))
                            .flex()
                            .items_center()
                            .gap_3()
                            .child(icon(IconName::FileText, 20.0, p.muted))
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .flex()
                                    .flex_col()
                                    .child(
                                        div()
                                            .text_size(px(13.0))
                                            .line_height(px(17.0))
                                            .truncate()
                                            .child(name),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(11.5))
                                            .line_height(px(15.0))
                                            .text_color(rgb(p.muted))
                                            .text_ellipsis_middle()
                                            .whitespace_nowrap()
                                            .overflow_hidden()
                                            .child(location.clone()),
                                    ),
                            );
                        file_control(
                            el,
                            ("recent-document", index),
                            description,
                            enabled,
                            p.accent,
                            cx,
                            move |v, w, cx| {
                                w.focus(&v.focus, cx);
                                v.begin_open(OpenSource::File(path.clone()), cx);
                            },
                        )
                        .when(enabled, |el| el.hover(move |s| s.bg(rgb(p.hover))))
                        .tooltip(move |w, cx| {
                            let location = location.clone();
                            Tooltip::element(move |_, _| {
                                div()
                                    .w(px(360.0))
                                    .whitespace_normal()
                                    .child(location.clone())
                            })
                            .build(w, cx)
                        })
                    })),
            )
    }

    fn file_info(&self) -> Div {
        let p = self.palette();
        let tr = |zh, en| self.shell.tr(zh, en);
        let Some(reader) = self.canvas.reader.as_ref() else {
            return div()
                .text_color(rgb(p.muted))
                .child(tr("当前没有打开文档。", "No document is open."));
        };
        let location = reader.source_path().map_or_else(
            || tr("尚未保存到文件", "No saved location").to_owned(),
            display_path,
        );
        joyoffice_ui::card(p)
            .max_w(px(620.0))
            .p_5()
            .flex()
            .flex_col()
            .gap_2()
            .child(
                div()
                    .text_size(px(17.0))
                    .child(if reader.summary.name == "Untitled" {
                        tr("未命名文档", "Untitled document").to_owned()
                    } else {
                        reader.summary.name.clone()
                    }),
            )
            .child(div().text_color(rgb(p.muted)).child(location))
            .child(format!(
                "{} {} · {}",
                reader.summary.pages.len(),
                tr("页", "pages"),
                if reader.is_dirty() {
                    tr("有未保存修改", "Unsaved changes")
                } else {
                    tr("无未保存修改", "No unsaved changes")
                }
            ))
    }

    fn file_recovery(&self, cx: &mut Context<Self>) -> Div {
        let tr = |zh, en| self.shell.tr(zh, en);
        let current = self
            .canvas
            .reader
            .as_ref()
            .and_then(|r| r.recovery_first())
            .is_some();
        let enabled = self.file_change_available();
        let legacy = self.legacy_recovery.clone();
        div()
            .flex()
            .flex_col()
            .gap_2()
            .max_w(px(360.0))
            .when(!current && legacy.is_none(), |el| {
                el.text_color(rgb(self.palette().muted)).child(tr(
                    "没有可用的恢复副本。",
                    "No recovery copies are available.",
                ))
            })
            .when(current, |el| {
                el.child(self.file_row(
                    "recover-latest",
                    tr("恢复最新副本", "Recover latest copy"),
                    IconName::RotateCcw,
                    enabled,
                    cx,
                    |v, w, cx| {
                        v.return_to_document(w, cx);
                        v.execute("file.recover_latest", w, cx);
                    },
                ))
            })
            .children(legacy.map(|path| {
                self.file_row(
                    "recover-legacy",
                    tr("恢复旧版副本", "Recover legacy copy"),
                    IconName::Clock,
                    enabled,
                    cx,
                    move |v, w, cx| {
                        w.focus(&v.focus, cx);
                        v.begin_open(OpenSource::LegacyRecovery(path.clone()), cx);
                    },
                )
            }))
    }

    #[cfg(windows)]
    fn file_print(&self, cx: &mut Context<Self>) -> Div {
        let tr = |zh, en| self.shell.tr(zh, en);
        let enabled = self.canvas.reader.is_some() && self.file_change_available();
        div()
            .flex()
            .flex_col()
            .gap_2()
            .max_w(px(360.0))
            .child(self.file_row(
                "print-preview",
                tr("打印预览…", "Print preview…"),
                IconName::FileText,
                enabled,
                cx,
                |v, w, cx| v.open_print_preview(w, cx),
            ))
            .child(self.file_row(
                "system-print",
                tr("打印…", "Print…"),
                IconName::Printer,
                enabled && !self.printing,
                cx,
                |v, w, cx| v.open_print(w, cx),
            ))
    }

    fn file_export(&self, cx: &mut Context<Self>) -> Div {
        use export::Format::{Archive, Copy, Epub, Png, Word2003};

        let p = self.palette();
        let tr = |zh, en| self.shell.tr(zh, en);
        let enabled = self.file_change_available()
            && self
                .canvas
                .reader
                .as_ref()
                .is_some_and(|r| !r.is_composing());
        let formats = [
            ("PDF 文档", "PDF document", Copy("pdf")),
            ("Word 文档", "Word document", Copy("docx")),
            ("Word XML 文档", "Word XML document", Copy("xml")),
            ("Word 2003 XML 文档", "Word 2003 XML document", Word2003),
            ("OpenDocument 文本", "OpenDocument Text", Copy("odt")),
            (
                "OpenDocument 文本模板",
                "OpenDocument Text Template",
                Copy("ott"),
            ),
            (
                "平面 OpenDocument 文本",
                "Flat OpenDocument Text",
                Copy("fodt"),
            ),
            ("RTF 格式", "Rich Text Format", Copy("rtf")),
            ("网页", "Web page", Copy("html")),
            ("Markdown", "Markdown", Copy("md")),
            ("LaTeX", "LaTeX", Copy("tex")),
            ("纯文本", "Plain text", Copy("txt")),
            ("页面图像", "Page image", Png(self.current_page())),
            ("EPUB 电子书", "EPUB e-book", Epub),
            ("PDF/A-2b 归档文档", "PDF/A-2b archival document", Archive),
        ];
        div()
            .flex()
            .flex_col()
            .gap_3()
            .child(div().text_color(rgb(p.muted)).child(tr(
                "以其他格式保存副本。需要打印时，可导出 PDF 并在 PDF 查看器中打印。",
                "Save a copy in another format. To print, export a PDF and print it from a PDF viewer.",
            )))
            .child(
                div().flex().flex_col().gap_2().max_w(px(320.0)).children(
                    formats.into_iter().enumerate().map(|(index, (zh, en, format))| {
                        let name = tr(zh, en);
                        let label = format!("{name} (*.{})", format.suffix());
                        let row = joyoffice_ui::control(p, enabled, false)
                            .h(px(34.0))
                            .px_2()
                            .justify_start()
                            .bg(rgb(p.surface))
                            .child(label.clone());
                        file_control(row, ("export-format", index), label, enabled, p.accent, cx,
                            move |v, w, cx| v.start_checked_export(format, name, w, cx))
                            .border_color(rgb(p.border))
                            .when(enabled, |el| el.hover(move |s| s.bg(rgb(p.hover))))
                    }),
                ),
            )
    }

    fn file_options(&self, cx: &mut Context<Self>) -> Div {
        let tr = |zh, en| self.shell.tr(zh, en);
        div()
            .flex()
            .flex_col()
            .gap_2()
            .max_w(px(360.0))
            .child(self.file_row(
                "file-theme",
                if self.shell.dark {
                    tr("切换为浅色外观", "Switch to light appearance")
                } else {
                    tr("切换为深色外观", "Switch to dark appearance")
                },
                IconName::SunMoon,
                true,
                cx,
                |v, w, cx| v.execute("view.theme", w, cx),
            ))
            .child(self.file_row(
                "file-language",
                tr("Switch to English", "切换为中文"),
                IconName::Globe,
                true,
                cx,
                |v, w, cx| v.execute("view.language", w, cx),
            ))
            .child(self.file_row(
                "file-general-options",
                tr("用户信息与单位…", "User information and units…"),
                IconName::Settings,
                self.canvas.reader.is_some() && self.file_change_available(),
                cx,
                |v, w, cx| {
                    v.return_to_document(w, cx);
                    v.advanced_action("general-options", w, cx);
                },
            ))
    }

    fn file_row(
        &self,
        id: &'static str,
        label: &'static str,
        symbol: IconName,
        enabled: bool,
        cx: &mut Context<Self>,
        run: impl Fn(&mut JoyWord, &mut Window, &mut Context<JoyWord>) + 'static,
    ) -> Stateful<Div> {
        let p = self.palette();
        let el = joyoffice_ui::control(p, enabled, false)
            .h(px(36.0))
            .justify_start()
            .child(icon(
                symbol,
                metrics::ICON,
                if enabled { p.accent } else { p.disabled },
            ))
            .child(label);
        file_control(el, id, label, enabled, p.accent, cx, run)
            .when(enabled, |el| el.hover(move |s| s.bg(rgb(p.hover))))
    }
}

fn file_control(
    el: Div,
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    enabled: bool,
    focus_color: u32,
    cx: &mut Context<JoyWord>,
    run: impl Fn(&mut JoyWord, &mut Window, &mut Context<JoyWord>) + 'static,
) -> Stateful<Div> {
    let el = el.id(id).role(Role::Button).aria_label(label);
    if !enabled {
        return el.a11y_synthetic_children(|b| b.parent_node().set_disabled());
    }
    let run = Rc::new(run);
    let click = run.clone();
    el.occlude()
        .tab_stop(true)
        .cursor_pointer()
        .border_1()
        .border_color(gpui::transparent_black())
        .focus_visible(move |s| s.border_color(rgb(focus_color)))
        .on_click(cx.listener(move |v, _, w, cx| click(v, w, cx)))
        .on_key_down(cx.listener(move |v, event: &KeyDownEvent, w, cx| {
            if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                run(v, w, cx);
                cx.stop_propagation();
            }
        }))
}

fn display_path(path: &Path) -> String {
    let display = path.to_string_lossy();
    #[cfg(windows)]
    if let Some(std::path::Component::Prefix(prefix)) = path.components().next() {
        use std::path::Prefix::{VerbatimDisk, VerbatimUNC};
        match prefix.kind() {
            VerbatimDisk(_) => return display[4..].to_owned(),
            VerbatimUNC(_, _) => return format!(r"\\{}", &display[8..]),
            _ => {}
        }
    }
    display.into_owned()
}

#[cfg(all(test, windows))]
mod tests {
    use super::display_path;
    use std::path::Path;

    #[test]
    fn recent_paths_hide_only_windows_disk_and_unc_verbatim_prefixes() {
        for (stored, shown) in [
            (r"\\?\C:\文档\样例.docx", r"C:\文档\样例.docx"),
            (
                r"\\?\UNC\server\share\letter.docx",
                r"\\server\share\letter.docx",
            ),
            (
                r"\\?\Volume{test}\letter.docx",
                r"\\?\Volume{test}\letter.docx",
            ),
            (r"C:\letter.docx", r"C:\letter.docx"),
        ] {
            assert_eq!(display_path(Path::new(stored)), shown);
        }
    }
}
