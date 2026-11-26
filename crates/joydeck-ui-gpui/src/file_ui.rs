//! Native file page with the current document, original templates and recent files.
use super::*;
use gpui_kit::component::{
    Icon, Selectable, Sizable,
    button::ButtonVariants,
    input::InputEvent,
    scroll::{Scrollbar, ScrollbarMode},
};
use joydeck_engine::{Command, ContentAction, Session};
use joyoffice_ui::{IconName as I, Palette, display_path};
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

fn template_display_name(locale: joyoffice_ui::UiLocale, internal: &str) -> &'static str {
    let key = match internal {
        "空白演示文稿" => "template.blank",
        "简洁简报" => "template.brief",
        "课程讲义" => "template.course",
        "项目计划" => "template.plan",
        _ => return crate::l10n::t(locale, "template.blank"),
    };
    crate::l10n::t(locale, key)
}

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

fn modified_label(locale: joyoffice_ui::UiLocale, modified: Option<SystemTime>) -> String {
    let Some(age) = modified.and_then(|time| SystemTime::now().duration_since(time).ok()) else {
        return "—".into();
    };
    match age.as_secs() {
        0..60 => crate::l10n::t(locale, "file.modified.just_now").into(),
        seconds @ 60..3600 => {
            let n = (seconds / 60).to_string();
            crate::l10n::tf(locale, "file.modified.minutes_ago", &[("count", &n)])
        }
        seconds @ 3600..86400 => {
            let n = (seconds / 3600).to_string();
            crate::l10n::tf(locale, "file.modified.hours_ago", &[("count", &n)])
        }
        seconds => {
            let n = (seconds / 86400).to_string();
            crate::l10n::tf(locale, "file.modified.days_ago", &[("count", &n)])
        }
    }
}

fn greeting(locale: joyoffice_ui::UiLocale) -> &'static str {
    let key = match joydeck_text::datetime::DateTime::now_local().map(|time| time.hour) {
        Some(0..=10) => "file.greeting.morning",
        Some(11..=12) => "file.greeting.noon",
        Some(13..=17) => "file.greeting.afternoon",
        Some(_) => "file.greeting.evening",
        None => "file.greeting.welcome",
    };
    crate::l10n::t(locale, key)
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
        let locale = self.locale;
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
                                populate_template_for(&mut session, name, locale)?;
                            }
                            let doc = Deck::from_model(name.into(), session.model().clone())?;
                            let mut raster =
                                doc.render(0, 352. / doc.info.width_pt, &Default::default())?;
                            for pixel in raster.pixels.as_chunks_mut::<4>().0 {
                                pixel.swap(0, 2);
                            }
                            image_of(
                                Raster {
                                    width: raster.width,
                                    height: raster.height,
                                    bgra: raster.pixels,
                                },
                                locale,
                            )
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
            self.t("file.state.dirty")
        } else if self.source.is_some() {
            self.t("file.state.saved")
        } else {
            self.t("file.state.unsaved")
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
            ("file-start", self.t("file.nav.home"), I::House, Page::Home),
            ("file-new", self.t("file.nav.new"), I::FilePlus, Page::New),
            (
                "file-open-page",
                self.t("file.nav.open"),
                I::FolderOpen,
                Page::Open,
            ),
        ] {
            top = top.child(
                nav_button(id, label, icon, self.file_page == page, p)
                    .on_click(cx.listener(move |s, _, w, cx| s.select_file_page(page, w, cx))),
            );
        }
        top = top
            .child(
                nav_button("file-share", self.t("file.nav.share"), I::Share, false, p)
                    .disabled(true)
                    .tooltip(self.t("file.nav.share_tip")),
            )
            .child(divider(p))
            .child(
                nav_button(
                    "file-info",
                    self.t("file.nav.info"),
                    I::Info,
                    self.file_page == Page::Info,
                    p,
                )
                .on_click(cx.listener(|s, _, w, cx| s.select_file_page(Page::Info, w, cx))),
            )
            .child(
                nav_button("file-save", self.t("file.nav.save"), I::Save, false, p)
                    .disabled(self.save_unavailable().is_some())
                    .on_click(cx.listener(|s, _, w, cx| s.save(&Save, w, cx))),
            )
            .child(
                nav_button(
                    "file-save-as",
                    self.t("file.nav.save_as"),
                    I::SaveAll,
                    false,
                    p,
                )
                .disabled(self.save_unavailable().is_some())
                .on_click(cx.listener(|s, _, w, cx| s.save_as(&SaveAs, w, cx))),
            );
        for (id, label, icon, page) in [
            (
                "file-print-page",
                self.t("file.nav.print"),
                I::Printer,
                Page::Print,
            ),
            (
                "file-export-page",
                self.t("file.nav.export"),
                I::FileOutput,
                Page::Export,
            ),
        ] {
            top = top.child(
                nav_button(id, label, icon, self.file_page == page, p)
                    .on_click(cx.listener(move |s, _, w, cx| s.select_file_page(page, w, cx))),
            );
        }
        top = top.child(
            nav_button(
                "file-close",
                self.t("file.nav.close"),
                I::FolderClosed,
                false,
                p,
            )
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
                            .accessibility_label(self.t("file.nav.back_aria"))
                            .tooltip(self.t("file.nav.back_tip"))
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
                nav_button(
                    "file-account",
                    self.t("file.nav.account"),
                    I::UserRound,
                    false,
                    p,
                )
                .disabled(true)
                .tooltip(self.t("file.nav.account_tip")),
            )
            .child(
                nav_button(
                    "file-options",
                    self.t("file.nav.options"),
                    I::Settings,
                    self.file_page == Page::Options,
                    p,
                )
                .on_click(cx.listener(|s, _, w, cx| s.select_file_page(Page::Options, w, cx))),
            )
            .child(
                nav_button("file-about", self.t("file.nav.about"), I::Info, false, p)
                    .on_click(|_, w, cx| crate::show_about(w, cx)),
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
                    .child(greeting(self.locale)),
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
                    .label(self.t("file.new_blank_window"))
                    .on_click(cx.listener(|s, _, _, cx| {
                        match super::open_window(s.dark, (1440., 900.), cx) {
                            Ok((handle, view)) => super::new_document(&view, handle, cx),
                            Err(error) => {
                                let err_s = error.to_string();
                                s.message = s.tf("file.new_window_failed", &[("error", &err_s)]);
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
                    .accessibility_label(self.t("file.toggle_templates_aria"))
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
                                    .child(self.t("file.nav.new")),
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
                        .accessibility_label(self.t("file.more_themes"))
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(12.))
                                .text_color(rgb(p.accent))
                                .child(div().text_size(px(14.)).child(self.t("file.more_themes")))
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
        let display_name = template_display_name(self.locale, name);
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
                    .child(self.tf("file.preview_unavailable", &[("error", error)])),
            );
        }
        Button::new(("file-template", index))
            .ghost()
            .w(px(216.))
            .h(px(158.))
            .p_0()
            .accessibility_label(display_name)
            .tooltip(display_name)
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
                            .child(display_name),
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
                    .child(self.t("file.nav.open")),
            )
            .child(
                Button::new("file-browse")
                    .ghost()
                    .icon(I::FolderOpen)
                    .label(self.t("file.browse_pc"))
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
            .child(recent_filter(
                "recent-tab",
                self.t("file.tab.recent"),
                true,
                p,
            ))
            .child(
                recent_filter("favorite-tab", self.t("file.tab.favorites"), false, p)
                    .disabled(true)
                    .tooltip(self.t("file.tab.favorites_tip")),
            )
            .child(
                recent_filter("shared-tab", self.t("file.tab.shared"), false, p)
                    .disabled(true)
                    .tooltip(self.t("file.tab.shared_tip")),
            )
            .child(div().flex_1())
            .child(
                div().w(relative(0.38)).min_w(px(260.)).child(
                    Input::new(&self.file_search)
                        .large()
                        .prefix(Icon::new(I::Search).size(px(21.)))
                        .aria_label(self.t("file.search_recent_aria"))
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
                .child(
                    div()
                        .flex_1()
                        .text_size(px(13.))
                        .child(self.t("file.col.name")),
                )
                .child(
                    div()
                        .w(px(142.))
                        .text_size(px(13.))
                        .child(self.t("file.col.modified")),
                ),
        );
        match &self.recent_paths {
            Ok(files) => {
                let mut count = 0;
                for (index, recent) in files.iter().enumerate() {
                    let label = display_path(&recent.path);
                    if !label.to_lowercase().contains(&query) {
                        continue;
                    }
                    count += 1;
                    list = list.child(self.recent_file_row(index, recent, label, p, cx));
                }
                if count == 0 {
                    list = list.child(
                        div()
                            .py_6()
                            .text_size(px(14.))
                            .text_color(rgb(p.muted))
                            .child(if query.is_empty() {
                                self.t("file.recent.empty")
                            } else {
                                self.t("file.recent.no_match")
                            }),
                    );
                }
            }
            Err(error) => {
                list = list.child(
                    div()
                        .py_6()
                        .child(self.tf("file.recent.error", &[("error", error)])),
                );
            }
        }
        div().flex().flex_col().child(header).child(list)
    }

    fn recent_file_row(
        &self,
        index: usize,
        recent: &joydeck_shell::storage::RecentFile,
        label: String,
        p: Palette,
        cx: &mut Context<Self>,
    ) -> Button {
        let path = recent.path.clone();
        let name = path
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        let directory = path.parent().map(display_path).unwrap_or_default();
        let accessible = self.tf("file.recent.open_aria", &[("label", &label)]);
        Button::new(("recent-file", index))
            .ghost()
            .w_full()
            .h(px(70.))
            .p_0()
            .rounded_none()
            .accessibility_label(accessible)
            .tooltip(label)
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
                            .child(modified_label(self.locale, recent.modified)),
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
                    .child(self.t("file.nav.info")),
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
                        .map_or_else(|| self.t("file.info.unsaved").into(), |p| display_path(p)),
                ),
            );
        if let Some(editor) = &self.editor {
            let model = editor.session.model();
            let count_s = model.slides.len().to_string();
            let width_s = format!("{:.0}", model.slide_size.width);
            let height_s = format!("{:.0}", model.slide_size.height);
            panel = panel.child(self.tf(
                "file.info.summary",
                &[
                    ("count", &count_s),
                    ("width", &width_s),
                    ("height", &height_s),
                ],
            ));
        }
        panel
            .child(
                Button::new("file-properties")
                    .ghost()
                    .icon(I::FilePenLine)
                    .label(self.t("file.info.properties"))
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
                    .label(self.t("file.info.compatibility"))
                    .on_click(cx.listener(|s, _, w, cx| s.report(&Report, w, cx))),
            )
            .child(
                Button::new("file-recover")
                    .ghost()
                    .icon(I::FolderClock)
                    .label(self.t("file.info.recover"))
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
                .child(self.t("file.nav.print")),
        );
        for (id, label, layout) in [
            (
                "print-slides",
                self.t("file.print.slides"),
                PageLayout::Slides,
            ),
            ("print-notes", self.t("file.print.notes"), PageLayout::Notes),
            (
                "print-handouts",
                self.t("file.print.handouts"),
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
                    .child(self.t("file.nav.export")),
            )
            .child(self.delivery_toolbar(cx))
    }

    fn file_options(&self, cx: &mut Context<Self>) -> Div {
        let p = self.palette();
        let view = cx.entity().downgrade();
        div()
            .flex()
            .flex_col()
            .gap_5()
            .child(
                div()
                    .text_size(px(30.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(self.t("file.nav.options")),
            )
            .child(joyoffice_ui::language_options_section(
                self.locale,
                p,
                360.0,
                move |locale, w, cx| {
                    let _ = view.update(cx, |s, cx| s.select_locale(locale, w, cx));
                },
            ))
            .child(
                div()
                    .text_size(px(18.))
                    .child(self.t("file.options.appearance")),
            )
            .child(
                div()
                    .text_color(rgb(p.muted))
                    .child(self.t("file.options.appearance_desc")),
            )
            .child(div().flex().flex_wrap().gap_2().max_w(px(360.)).children(
                joyoffice_ui::ThemePreset::ALL.into_iter().map(|theme| {
                    Button::new(SharedString::from(format!("appearance-{}", theme.id())))
                        .w(px(172.))
                        .h_auto()
                        .p_0()
                        .selected(self.theme == theme)
                        .toggled(self.theme == theme)
                        .accessibility_label(joyoffice_ui::theme_label(theme, self.locale))
                        .child(joyoffice_ui::theme_preview_for(
                            theme,
                            joyoffice_ui::Brand::Deck,
                            self.locale,
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
        let result = template_editor_for(name, self.locale, joydeck_shell::editor::recovery_root());
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
                let display = template_display_name(self.locale, name);
                self.message = self.tf("file.template_created", &[("name", display)]);
            }
            Err(error) => self.message = error,
        }
        cx.notify();
    }
}

#[cfg_attr(not(test), allow(dead_code))]
fn template_editor(
    name: &str,
    root: std::path::PathBuf,
) -> Result<joydeck_shell::editor::Editor, String> {
    template_editor_for(name, joyoffice_ui::UiLocale::ZhCn, root)
}

fn template_editor_for(
    name: &str,
    locale: joyoffice_ui::UiLocale,
    root: std::path::PathBuf,
) -> Result<joydeck_shell::editor::Editor, String> {
    let mut editor = joydeck_shell::editor::Editor::new(root)?;
    populate_template_for(&mut editor.session, name, locale)?;
    let snapshot = editor.session.snapshot();
    editor.session = Session::new(snapshot.model.as_ref().clone(), snapshot.origins, false);
    Ok(editor)
}

fn populate_template_for(
    session: &mut Session,
    name: &str,
    locale: joyoffice_ui::UiLocale,
) -> Result<(), String> {
    let t = |key: &'static str| crate::l10n::t(locale, key);
    let pages: [(&str, &str); 3] = match name {
        "简洁简报" => [
            (t("template.brief.s1_title"), t("template.brief.s1_body")),
            (t("template.brief.s2_title"), t("template.brief.s2_body")),
            (t("template.brief.s3_title"), t("template.brief.s3_body")),
        ],
        "课程讲义" => [
            (t("template.course.s1_title"), t("template.course.s1_body")),
            (t("template.course.s2_title"), t("template.course.s2_body")),
            (t("template.course.s3_title"), t("template.course.s3_body")),
        ],
        "项目计划" => [
            (t("template.plan.s1_title"), t("template.plan.s1_body")),
            (t("template.plan.s2_title"), t("template.plan.s2_body")),
            (t("template.plan.s3_title"), t("template.plan.s3_body")),
        ],
        _ => return Err(t("file.template_unknown").to_string()),
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
    use super::{template_editor, template_editor_for};
    use joyoffice_ui::UiLocale;

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
        for (name, titles) in [
            (
                "简洁简报",
                ["Executive Brief", "Key Takeaways", "Action Plan"],
            ),
            ("课程讲义", ["Course Topic", "Core Concepts", "Summary"]),
            (
                "项目计划",
                ["Project Plan", "Milestones", "Risks & Resources"],
            ),
        ] {
            let result = template_editor_for(name, UiLocale::En, root.join("recovery-en"));
            assert!(
                result.is_ok(),
                "english template {name} failed: {:?}",
                result.err()
            );
            if let Ok(editor) = result {
                assert_eq!(editor.session.model().slides.len(), 3);
                for (slide, title) in editor.session.model().slides.iter().zip(titles) {
                    assert!(slide.shapes.iter().any(|shape| {
                        shape.text.as_ref().is_some_and(|text| text.text() == title)
                    }));
                }
            }
        }
        std::fs::remove_dir_all(root).unwrap();
    }
}
