//! Native file page with the current document, original templates and recent files.
use super::*;
use gpui_kit::component::{
    Icon, Selectable, Sizable,
    button::ButtonVariants,
    input::InputEvent,
    scroll::{Scrollbar, ScrollbarMode},
};
use joydeck_engine::{Command, ContentAction, Session};
use joyoffice_ui::{IconName as I, Palette};
use std::time::SystemTime;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Page {
    Home,
    New,
    Open,
    Info,
    Print,
    Export,
    Options,
}

const TEMPLATES: [&str; 4] = ["空白演示文稿", "简洁简报", "课程讲义", "项目计划"];

fn nav_button(
    id: &'static str,
    label: &'static str,
    icon: I,
    selected: bool,
    p: Palette,
) -> Button {
    Button::new(id)
        .ghost()
        .w_full()
        .h(px(49.))
        .p_0()
        .rounded_none()
        .accessibility_label(label)
        .selected(selected)
        .bg(rgb(p.chrome))
        .child(
            div()
                .flex()
                .items_center()
                .w_full()
                .h_full()
                .gap(px(13.))
                .pl(px(20.))
                .pr(px(10.))
                .border_l(px(3.))
                .border_color(if selected {
                    rgb(p.accent).into()
                } else {
                    transparent_black()
                })
                .when(selected, |d| d.text_color(rgb(p.accent)))
                .when(selected, |d| d.font_weight(FontWeight::SEMIBOLD))
                .child(Icon::new(icon).size(px(20.)))
                .child(div().text_size(px(14.)).child(label)),
        )
}

fn divider(p: Palette) -> Div {
    div()
        .mx(px(26.))
        .my(px(12.))
        .h(px(1.))
        .flex_shrink_0()
        .bg(rgb(p.border))
}

fn recent_filter(id: &'static str, label: &'static str, selected: bool, p: Palette) -> Button {
    Button::new(id)
        .ghost()
        .w(px(104.))
        .h(px(38.))
        .rounded(px(22.))
        .border_1()
        .border_color(rgb(if selected { p.accent } else { p.border }))
        .bg(rgb(if selected { p.accent } else { p.surface }))
        .accessibility_label(label)
        .child(
            div()
                .text_size(px(18.))
                .font_weight(FontWeight::NORMAL)
                .text_color(rgb(if selected { p.on_accent } else { p.text }))
                .child(label),
        )
}

fn modified_label(modified: Option<SystemTime>) -> String {
    let Some(age) = modified.and_then(|time| SystemTime::now().duration_since(time).ok()) else {
        return "—".into();
    };
    match age.as_secs() {
        0..60 => "刚刚".into(),
        seconds @ 60..3600 => format!("{} 分钟前", seconds / 60),
        seconds @ 3600..86400 => format!("{} 小时前", seconds / 3600),
        seconds => format!("{} 天前", seconds / 86400),
    }
}

fn greeting() -> &'static str {
    match joydeck_text::datetime::DateTime::now_local().map(|time| time.hour) {
        Some(0..=10) => "早上好",
        Some(11..=12) => "中午好",
        Some(13..=17) => "下午好",
        Some(_) => "晚上好",
        None => "欢迎使用 JoyDeck",
    }
}

impl Viewer {
    pub(super) fn backstage_visible(&self) -> bool {
        self.backstage && !self.ribbon_collapsed
    }

    pub(super) fn open_file_page(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        if self.composing()
            || self
                .notes_input
                .update(cx, |input, cx| input.marked_text_range(w, cx).is_some())
        {
            return;
        }
        self.commit_notes(w, cx);
        self.end_inline(cx);
        self.backstage = true;
        self.ribbon_collapsed = false;
        self.file_page = Page::Home;
        self.file_scroll.set_offset(point(px(0.), px(0.)));
        self.file_search
            .update(cx, |input, cx| input.set_value("", w, cx));
        self.focus.focus(w, cx);
        self.refresh_recent(cx);
        self.request_template_previews(cx);
        cx.notify();
    }

    pub(super) fn close_file_page(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        self.backstage = false;
        self.focus.focus(w, cx);
        cx.notify();
    }

    fn select_file_page(&mut self, page: Page, w: &mut Window, cx: &mut Context<Self>) {
        self.file_page = page;
        self.file_scroll.set_offset(point(px(0.), px(0.)));
        self.focus.focus(w, cx);
        cx.notify();
    }

    pub(super) fn install_file_search(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        self.file_search_subscription =
            Some(
                cx.subscribe_in(&self.file_search, w, |_, _, event: &InputEvent, _, cx| {
                    if matches!(event, InputEvent::Change) {
                        cx.notify();
                    }
                }),
            );
    }

    pub(super) fn refresh_recent(&mut self, cx: &mut Context<Self>) {
        let root = joydeck_shell::editor::recovery_root();
        self.recent_task = Some(cx.spawn(async move |view, cx| {
            let result = cx
                .background_executor()
                .spawn(async move { joydeck_shell::storage::recent_file_details(&root) })
                .await;
            let _ = view.update(cx, |s, cx| {
                s.recent_paths = result;
                cx.notify();
            });
        }));
    }

    fn request_template_previews(&mut self, cx: &mut Context<Self>) {
        if !self.template_previews.is_empty() || self.template_preview_task.is_some() {
            return;
        }
        self.template_preview_task = Some(cx.spawn(async move |view, cx| {
            let previews = cx
                .background_executor()
                .spawn(async move {
                    TEMPLATES
                        .into_iter()
                        .map(|name| {
                            let mut session = Session::new(
                                joydeck_model::Presentation::default(),
                                Default::default(),
                                false,
                            );
                            if name != TEMPLATES[0] {
                                populate_template(&mut session, name)?;
                            }
                            let doc = Deck::from_model(name.into(), session.model().clone())?;
                            let mut raster =
                                doc.render(0, 352. / doc.info.width_pt, &Default::default())?;
                            for pixel in raster.pixels.as_chunks_mut::<4>().0 {
                                pixel.swap(0, 2);
                            }
                            image_of(Raster {
                                width: raster.width,
                                height: raster.height,
                                bgra: raster.pixels,
                            })
                        })
                        .collect::<Vec<_>>()
                })
                .await;
            let _ = view.update(cx, |s, cx| {
                s.template_previews = previews;
                s.template_preview_task = None;
                cx.notify();
            });
        }));
    }

    #[inline(never)]
    pub(super) fn file_backstage(&self, w: &Window, cx: &mut Context<Self>) -> AnyElement {
        let p = self.palette();
        let compact = f32::from(w.viewport_size().width) < 1100.;
        let content = match self.file_page {
            Page::Home => div().child(self.file_home(cx)),
            Page::New => div().child(self.file_new(cx)),
            Page::Open => div().child(self.file_open(cx)),
            Page::Info => div().child(self.file_info(cx)),
            Page::Print => div().child(self.file_print(cx)),
            Page::Export => div().child(self.file_export(cx)),
            Page::Options => div().child(self.file_options(cx)),
        };
        div()
            .flex_1()
            .min_h_0()
            .flex()
            .flex_col()
            .bg(rgb(p.surface))
            .text_color(rgb(p.text))
            .child(self.file_header(p))
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .child(self.file_navigation(p, cx))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .relative()
                            .child(
                                div()
                                    .id("file-page-content")
                                    .size_full()
                                    .overflow_y_scroll()
                                    .track_scroll(&self.file_scroll)
                                    .px(px(if compact { 28. } else { 60. }))
                                    .pt(px(12.))
                                    .pb(px(40.))
                                    .child(content),
                            )
                            .child(
                                Scrollbar::vertical(&self.file_scroll).mode(ScrollbarMode::Always),
                            ),
                    ),
            )
            .into_any_element()
    }

    fn file_header(&self, p: Palette) -> impl IntoElement {
        let status = if self.editor.as_ref().is_some_and(|e| e.session.dirty()) {
            "有未保存的更改"
        } else if self.source.is_some() {
            "已保存到这台电脑"
        } else {
            "未保存"
        };
        div()
            .id("file-title-bar")
            .h(px(62.))
            .flex_shrink_0()
            .bg(rgb(p.chrome))
            .flex()
            .items_center()
            .gap(px(20.))
            .px(px(20.))
            .when(cfg!(target_os = "macos"), |d| d.pl(px(90.)))
            .child(
                Icon::new(I::Presentation)
                    .size(px(22.))
                    .text_color(rgb(p.accent)),
            )
            .child(
                div()
                    .id("file-document-title")
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .flex()
                    .items_center()
                    .window_control_area(WindowControlArea::Drag)
                    .child(
                        div()
                            .text_size(px(14.))
                            .text_ellipsis()
                            .child(format!("{} · {status}", self.document_name())),
                    ),
            )
            .child(
                div()
                    .max_w(px(340.))
                    .text_size(px(12.))
                    .text_color(rgb(p.muted))
                    .text_ellipsis()
                    .child(self.message.clone()),
            )
    }

    #[inline(never)]
    fn file_navigation(&self, p: Palette, cx: &mut Context<Self>) -> Div {
        let mut top = div().flex().flex_col().flex_shrink_0();
        for (id, label, icon, page) in [
            ("file-start", "开始", I::House, Page::Home),
            ("file-new", "新建", I::FilePlus, Page::New),
            ("file-open-page", "打开", I::FolderOpen, Page::Open),
        ] {
            top = top.child(
                nav_button(id, label, icon, self.file_page == page, p)
                    .on_click(cx.listener(move |s, _, w, cx| s.select_file_page(page, w, cx))),
            );
        }
        top = top
            .child(
                nav_button("file-share", "共享", I::Share, false, p)
                    .disabled(true)
                    .tooltip("暂不支持共享"),
            )
            .child(divider(p))
            .child(
                nav_button(
                    "file-info",
                    "信息",
                    I::Info,
                    self.file_page == Page::Info,
                    p,
                )
                .on_click(cx.listener(|s, _, w, cx| s.select_file_page(Page::Info, w, cx))),
            )
            .child(
                nav_button("file-save", "保存", I::Save, false, p)
                    .disabled(self.save_unavailable().is_some())
                    .on_click(cx.listener(|s, _, w, cx| s.save(&Save, w, cx))),
            )
            .child(
                nav_button("file-save-as", "另存为", I::SaveAll, false, p)
                    .disabled(self.save_unavailable().is_some())
                    .on_click(cx.listener(|s, _, w, cx| s.save_as(&SaveAs, w, cx))),
            );
        for (id, label, icon, page) in [
            ("file-print-page", "打印", I::Printer, Page::Print),
            ("file-export-page", "导出", I::FileOutput, Page::Export),
        ] {
            top = top.child(
                nav_button(id, label, icon, self.file_page == page, p)
                    .on_click(cx.listener(move |s, _, w, cx| s.select_file_page(page, w, cx))),
            );
        }
        top = top.child(
            nav_button("file-close", "关闭", I::FolderClosed, false, p)
                .on_click(cx.listener(|s, _, w, cx| s.request_close(&CloseEditor, w, cx))),
        );
        div()
            .w(px(200.))
            .flex_shrink_0()
            .h_full()
            .bg(rgb(p.chrome))
            .flex()
            .flex_col()
            .child(
                div()
                    .h(px(58.))
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .pl(px(18.))
                    .child(
                        Button::new("file-back")
                            .ghost()
                            .child(Icon::new(I::CircleArrowLeft).size(px(28.)))
                            .w(px(28.))
                            .h(px(28.))
                            .p_0()
                            .accessibility_label("返回演示文稿")
                            .tooltip("返回 · Esc")
                            .on_click(cx.listener(|s, _, w, cx| s.close_file_page(w, cx))),
                    ),
            )
            .child(
                div()
                    .id("file-nav-items")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .child(top),
            )
            .child(divider(p))
            .child(
                nav_button("file-account", "帐户", I::UserRound, false, p)
                    .disabled(true)
                    .tooltip("暂不支持帐户登录"),
            )
            .child(
                nav_button(
                    "file-options",
                    "选项",
                    I::Settings,
                    self.file_page == Page::Options,
                    p,
                )
                .on_click(cx.listener(|s, _, w, cx| s.select_file_page(Page::Options, w, cx))),
            )
    }

    #[inline(never)]
    fn file_home(&self, cx: &mut Context<Self>) -> Div {
        div()
            .flex()
            .flex_col()
            .child(
                div()
                    .h(px(44.))
                    .text_size(px(30.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(greeting()),
            )
            .child(self.file_templates(true, cx))
            .child(div().mt(px(58.)).child(self.recent_panel(cx)))
    }

    #[inline(never)]
    fn file_new(&self, cx: &mut Context<Self>) -> Div {
        div()
            .flex()
            .flex_col()
            .gap_6()
            .child(self.file_templates(false, cx))
            .child(
                Button::new("new-window")
                    .ghost()
                    .icon(I::PanelsTopLeft)
                    .label("在新窗口中新建空白文稿")
                    .on_click(cx.listener(|s, _, _, cx| {
                        match super::open_window(s.dark, (1440., 900.), cx) {
                            Ok((handle, view)) => super::new_document(&view, handle, cx),
                            Err(error) => {
                                s.message = format!("无法新建窗口：{error}");
                                cx.notify();
                            }
                        }
                    })),
            )
    }

    #[inline(never)]
    fn file_templates(&self, collapsible: bool, cx: &mut Context<Self>) -> Div {
        let p = self.palette();
        let heading = div()
            .flex()
            .items_center()
            .h(px(44.))
            .child(
                Button::new("file-templates-toggle")
                    .ghost()
                    .p_0()
                    .h(px(36.))
                    .accessibility_label("展开或折叠新建模板")
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(10.))
                            .child(
                                Icon::new(if self.file_templates_expanded || !collapsible {
                                    I::ChevronDown
                                } else {
                                    I::ChevronRight
                                })
                                .size(px(15.)),
                            )
                            .child(
                                div()
                                    .text_size(px(23.))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child("新建"),
                            ),
                    )
                    .on_click(cx.listener(move |s, _, _, cx| {
                        if collapsible {
                            s.file_templates_expanded = !s.file_templates_expanded;
                            cx.notify();
                        }
                    })),
            )
            .child(div().flex_1())
            .when(collapsible, |d| {
                d.child(
                    Button::new("file-more-templates")
                        .ghost()
                        .accessibility_label("更多主题")
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(12.))
                                .text_color(rgb(p.accent))
                                .child(div().text_size(px(14.)).child("更多主题"))
                                .child(Icon::new(I::ArrowRight).size(px(20.))),
                        )
                        .on_click(cx.listener(|s, _, w, cx| s.select_file_page(Page::New, w, cx))),
                )
            });
        div().mt(px(13.)).flex().flex_col().child(heading).when(
            !collapsible || self.file_templates_expanded,
            |d| {
                d.child(
                    div()
                        .id("template-cards")
                        .mt(px(30.))
                        .flex()
                        .flex_wrap()
                        .justify_between()
                        .gap(px(24.))
                        .children(
                            TEMPLATES
                                .into_iter()
                                .enumerate()
                                .map(|(index, name)| self.template_card(index, name, p, cx)),
                        ),
                )
            },
        )
    }

    fn template_card(
        &self,
        index: usize,
        name: &'static str,
        p: Palette,
        cx: &mut Context<Self>,
    ) -> Button {
        let preview = self.template_previews.get(index);
        let mut image = div()
            .w(px(176.))
            .h(px(100.))
            .border_1()
            .border_color(rgb(p.border))
            .bg(rgb(0xffffff));
        if let Some(Ok(preview)) = preview {
            image = image.child(
                img(preview.clone())
                    .w_full()
                    .h_full()
                    .object_fit(ObjectFit::Contain),
            );
        } else if let Some(Err(error)) = preview {
            image = image.child(
                div()
                    .p_2()
                    .text_size(px(12.))
                    .text_color(rgb(p.muted))
                    .child(format!("预览不可用：{error}")),
            );
        }
        Button::new(("file-template", index))
            .ghost()
            .w(px(216.))
            .h(px(158.))
            .p_0()
            .accessibility_label(name)
            .tooltip(name)
            .disabled(self.loading || self.composing() || self.is_saving())
            .child(
                div()
                    .size_full()
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap(px(20.))
                    .pt(px(4.))
                    .child(image)
                    .child(
                        div()
                            .text_size(px(14.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(name),
                    ),
            )
            .on_click(cx.listener(move |s, _, w, cx| {
                if index == 0 {
                    s.new_document(&NewDocument, w, cx);
                } else {
                    s.new_template(name, w, cx);
                }
            }))
    }

    fn file_open(&self, cx: &mut Context<Self>) -> Div {
        div()
            .flex()
            .flex_col()
            .gap_6()
            .child(
                div()
                    .text_size(px(30.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child("打开"),
            )
            .child(
                Button::new("file-browse")
                    .ghost()
                    .icon(I::FolderOpen)
                    .label("浏览这台电脑")
                    .on_click(cx.listener(|s, _, w, cx| s.open(&Open, w, cx))),
            )
            .child(self.recent_panel(cx))
    }

    #[inline(never)]
    fn recent_panel(&self, cx: &mut Context<Self>) -> Div {
        let p = self.palette();
        let query = self.file_search.read(cx).value().trim().to_lowercase();
        let header = div()
            .flex()
            .items_center()
            .flex_wrap()
            .gap_3()
            .child(recent_filter("recent-tab", "最近", true, p))
            .child(
                recent_filter("favorite-tab", "收藏夹", false, p)
                    .disabled(true)
                    .tooltip("暂不支持收藏文件"),
            )
            .child(
                recent_filter("shared-tab", "与我共享", false, p)
                    .disabled(true)
                    .tooltip("暂不支持共享文件"),
            )
            .child(div().flex_1())
            .child(
                div().w(relative(0.38)).min_w(px(260.)).child(
                    Input::new(&self.file_search)
                        .large()
                        .prefix(Icon::new(I::Search).size(px(21.)))
                        .aria_label("搜索最近文件")
                        .w_full(),
                ),
            );
        let mut list = div().flex().flex_col().mt(px(13.)).child(
            div()
                .flex()
                .items_center()
                .h(px(32.))
                .border_b_1()
                .border_color(rgb(p.border))
                .child(div().flex_1().text_size(px(13.)).child("名称"))
                .child(div().w(px(142.)).text_size(px(13.)).child("已修改日期")),
        );
        match &self.recent_paths {
            Ok(files) => {
                let mut count = 0;
                for (index, recent) in files.iter().enumerate() {
                    if !recent
                        .path
                        .to_string_lossy()
                        .to_lowercase()
                        .contains(&query)
                    {
                        continue;
                    }
                    count += 1;
                    list = list.child(self.recent_file_row(index, recent, p, cx));
                }
                if count == 0 {
                    list = list.child(
                        div()
                            .py_6()
                            .text_size(px(14.))
                            .text_color(rgb(p.muted))
                            .child(if query.is_empty() {
                                "打开或保存文稿后，会在这里显示最近文件。"
                            } else {
                                "没有找到匹配的文件。"
                            }),
                    );
                }
            }
            Err(error) => {
                list = list.child(div().py_6().child(format!("最近文件记录不可读取：{error}")));
            }
        }
        div().flex().flex_col().child(header).child(list)
    }

    fn recent_file_row(
        &self,
        index: usize,
        recent: &joydeck_shell::storage::RecentFile,
        p: Palette,
        cx: &mut Context<Self>,
    ) -> Button {
        let path = recent.path.clone();
        let name = path
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        let directory = path
            .parent()
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_default();
        let accessible = format!("打开最近文件 {}", path.display());
        Button::new(("recent-file", index))
            .ghost()
            .w_full()
            .h(px(70.))
            .p_0()
            .rounded_none()
            .accessibility_label(accessible)
            .tooltip(path.to_string_lossy().into_owned())
            .child(
                div()
                    .size_full()
                    .font_weight(FontWeight::NORMAL)
                    .flex()
                    .items_center()
                    .gap(px(17.))
                    .px(px(12.))
                    .border_b_1()
                    .border_color(rgb(p.border))
                    .child(
                        Icon::new(I::FileSliders)
                            .size(px(31.))
                            .text_color(rgb(p.accent)),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .gap(px(1.))
                            .child(div().text_size(px(18.)).text_ellipsis().child(name))
                            .child(
                                div()
                                    .text_size(px(13.))
                                    .text_color(rgb(p.muted))
                                    .text_ellipsis()
                                    .child(directory),
                            ),
                    )
                    .child(
                        div()
                            .w(px(130.))
                            .text_size(px(13.))
                            .child(modified_label(recent.modified)),
                    ),
            )
            .on_click(cx.listener(move |s, _, w, cx| s.open_path(path.clone(), w, cx)))
    }

    #[inline(never)]
    fn file_info(&self, cx: &mut Context<Self>) -> Div {
        let p = self.palette();
        let mut panel = div()
            .flex()
            .flex_col()
            .gap_5()
            .child(
                div()
                    .text_size(px(30.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child("信息"),
            )
            .child(
                div()
                    .text_size(px(21.))
                    .child(self.document_name().to_owned()),
            )
            .child(
                div().text_size(px(14.)).text_color(rgb(p.muted)).child(
                    self.source
                        .as_ref()
                        .map_or("文稿尚未保存".into(), |p| p.display().to_string()),
                ),
            );
        if let Some(editor) = &self.editor {
            let model = editor.session.model();
            panel = panel.child(format!(
                "{} 张幻灯片 · 页面尺寸 {:.0} × {:.0} 磅",
                model.slides.len(),
                model.slide_size.width,
                model.slide_size.height
            ));
        }
        panel
            .child(
                Button::new("file-properties")
                    .ghost()
                    .icon(I::FilePenLine)
                    .label("文稿属性")
                    .disabled(self.editor.is_none())
                    .on_click(
                        cx.listener(|s, _, w, cx| {
                            s.content_form(content_ui::Form::Metadata, w, cx)
                        }),
                    ),
            )
            .child(
                Button::new("file-compatibility")
                    .ghost()
                    .icon(I::FileCheck)
                    .label("兼容性检查")
                    .on_click(cx.listener(|s, _, w, cx| s.report(&Report, w, cx))),
            )
            .child(
                Button::new("file-recover")
                    .ghost()
                    .icon(I::FolderClock)
                    .label("恢复未保存文稿")
                    .on_click(cx.listener(|s, _, w, cx| s.recover(w, cx))),
            )
    }

    #[inline(never)]
    fn file_print(&self, cx: &mut Context<Self>) -> Div {
        use joydeck_shell::output::PageLayout;
        let mut panel = div().flex().flex_col().gap_5().child(
            div()
                .text_size(px(30.))
                .font_weight(FontWeight::SEMIBOLD)
                .child("打印"),
        );
        for (id, label, layout) in [
            ("print-slides", "打印幻灯片", PageLayout::Slides),
            ("print-notes", "打印备注页", PageLayout::Notes),
            (
                "print-handouts",
                "打印讲义",
                PageLayout::Handouts { per_page: 3 },
            ),
        ] {
            panel = panel.child(
                Button::new(id)
                    .ghost()
                    .icon(I::Printer)
                    .label(label)
                    .disabled(self.editor.is_none() || self.delivery_busy)
                    .on_click(
                        cx.listener(move |s, _, w, cx| s.delivery_form("print", layout, w, cx)),
                    ),
            );
        }
        panel
    }

    #[inline(never)]
    fn file_export(&self, cx: &mut Context<Self>) -> Div {
        div()
            .flex()
            .flex_col()
            .gap_5()
            .child(
                div()
                    .text_size(px(30.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child("导出"),
            )
            .child(self.delivery_toolbar(cx))
    }

    fn file_options(&self, cx: &mut Context<Self>) -> Div {
        div()
            .flex()
            .flex_col()
            .gap_5()
            .child(
                div()
                    .text_size(px(30.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child("选项"),
            )
            .child(div().text_size(px(18.)).child("外观"))
            .child(
                div()
                    .text_color(rgb(self.palette().muted))
                    .child("仅更改界面外观，不影响演示文稿内容。"),
            )
            .child(div().flex().flex_wrap().gap_2().max_w(px(360.)).children(
                joyoffice_ui::ThemePreset::ALL.into_iter().map(|theme| {
                    Button::new(SharedString::from(format!("appearance-{}", theme.id())))
                        .w(px(172.))
                        .h_auto()
                        .p_0()
                        .selected(self.theme == theme)
                        .toggled(self.theme == theme)
                        .accessibility_label(theme.label(true))
                        .child(joyoffice_ui::theme_preview(
                            theme,
                            joyoffice_ui::Brand::Deck,
                            true,
                        ))
                        .on_click(cx.listener(move |s, _, w, cx| s.select_theme(theme, w, cx)))
                }),
            ))
    }
    pub(super) fn new_template(
        &mut self,
        name: &'static str,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.confirm_replace(document_ui::ReplaceDocument::Template(name), w, cx) {
            return;
        }
        let result = template_editor(name, joydeck_shell::editor::recovery_root());
        match result {
            Ok(editor) => {
                self.stop_read();
                self.attach_new_editor(editor, cx);
                self.page = 0;
                self.zoom = None;
                self.cell = None;
                self.cell_range = None;
                self.slide_selection.clear();
                self.slide_anchor = None;
                self.backstage = false;
                self.generation += 1;
                self.sync_editor(w, cx);
                self.checkpoint(cx);
                self.message = format!("已从“{name}”新建，请另存文稿");
            }
            Err(error) => self.message = error,
        }
        cx.notify();
    }
}

fn template_editor(
    name: &str,
    root: std::path::PathBuf,
) -> Result<joydeck_shell::editor::Editor, String> {
    let mut editor = joydeck_shell::editor::Editor::new(root)?;
    populate_template(&mut editor.session, name)?;
    let snapshot = editor.session.snapshot();
    editor.session = Session::new(snapshot.model.as_ref().clone(), snapshot.origins, false);
    Ok(editor)
}

fn populate_template(session: &mut Session, name: &str) -> Result<(), String> {
    let pages: &[(&str, &str)] = match name {
        "简洁简报" => &[
            ("项目简报", "主题 · 日期 · 汇报人"),
            ("关键结论", "结论一\n结论二\n下一步行动"),
            ("行动计划", "任务 · 负责人 · 完成时间"),
        ],
        "课程讲义" => &[
            ("课程主题", "学习目标与适用对象"),
            ("核心概念", "概念说明\n原创示例\n练习与讨论"),
            ("课程回顾", "重点总结\n课后练习"),
        ],
        "项目计划" => &[
            ("项目计划", "项目目标与预期成果"),
            ("里程碑", "启动\n实施\n验收"),
            ("风险与资源", "主要风险\n应对措施\n所需资源"),
        ],
        _ => return Err("未知模板".to_string()),
    };
    let ids = session.model().slides[0]
        .shapes
        .iter()
        .map(|s| s.id)
        .collect();
    session.select(0, ids)?;
    session.execute(Command::Delete)?;
    let mut commands = vec![Command::Content {
        action: ContentAction::ThemeAccent {
            slot: 1,
            rgb: if name == "课程讲义" {
                0x268270
            } else {
                0xd95a1a
            },
        },
    }];
    for (index, (title, body)) in pages.iter().enumerate() {
        if index > 0 {
            commands.push(Command::NewSlide);
        }
        commands.push(Command::InsertText {
            x: 72.,
            y: 70.,
            text: (*title).into(),
        });
        commands.push(Command::InsertText {
            x: 72.,
            y: 170.,
            text: (*body).into(),
        });
    }
    session.execute(Command::Batch { commands })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::template_editor;
    #[test]
    fn starter_templates_create_save_and_reopen_all_three_pages() {
        let root = std::env::temp_dir().join(format!(
            "joydeck-template-roundtrip-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&root).unwrap();
        for (name, titles) in [
            ("简洁简报", ["项目简报", "关键结论", "行动计划"]),
            ("课程讲义", ["课程主题", "核心概念", "课程回顾"]),
            ("项目计划", ["项目计划", "里程碑", "风险与资源"]),
        ] {
            let mut editor = template_editor(name, root.join("recovery")).unwrap();
            assert!(editor.session.dirty());
            assert_eq!(editor.session.model().slides.len(), 3);
            for (slide, title) in editor.session.model().slides.iter().zip(titles) {
                assert!(
                    slide.shapes.iter().any(|shape| {
                        shape.text.as_ref().is_some_and(|text| text.text() == title)
                    })
                );
            }
            let path = root.join(format!("{name}.joydeck"));
            let saved = editor.save_job(path.clone()).unwrap().run().unwrap();
            editor.apply_saved(saved);
            assert!(!editor.session.dirty());
            let reopened =
                joydeck_shell::editor::Editor::open(&path, root.join("recovery")).unwrap();
            assert_eq!(
                serde_json::to_value(editor.session.model()).unwrap(),
                serde_json::to_value(reopened.session.model()).unwrap()
            );
            assert!(!reopened.session.dirty());
        }
        std::fs::remove_dir_all(root).unwrap();
    }
}
