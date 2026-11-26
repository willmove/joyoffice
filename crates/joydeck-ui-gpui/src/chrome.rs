//! JoyDeck native editor chrome built from GPUI Kit controls and the JoyOffice palette.
use super::ribbon_reference::{
    BLUE, GREEN, gallery, ribbon_checks, small_unavailable, unavailable, unavailable_row,
};
use super::ribbon_tabs::{RibbonTab, TabAction};
use super::*;
use crate::l10n::{presentation_theme_hint, presentation_theme_name, t, tf};
use gpui_kit::component::{Icon, Selectable, Sizable, button::ButtonVariants, popover::Popover};
use joydeck_engine::{AlignEdge, Command, DistributeAxis};
use joyoffice_ui::{IconName as I, Palette, UiLocale};

pub(super) fn small(id: &'static str, icon: I, label: &'static str) -> Button {
    Button::new(id)
        .ghost()
        .xsmall()
        .icon(icon)
        .label(label)
        .h(px(17.))
        .text_size(px(11.))
        .rounded(px(3.))
}
pub(super) const RIBBON_HEIGHT: f32 = 66.;
const RIBBON_OVERFLOW_WIDTH: f32 = 1280.;
const RIBBON_SCROLL_STEP: f32 = 240.;

pub(super) fn tool(id: &'static str, icon: I, label: &'static str, tint: u32) -> Button {
    let lines = label.split('\n');
    let width = lines
        .clone()
        .map(|line| {
            line.chars()
                .map(|ch| if ch.is_ascii() { 6.5_f32 } else { 11.0_f32 })
                .sum::<f32>()
        })
        .fold(0.0_f32, f32::max);
    let accessible = if label.is_ascii() {
        label.replace('\n', " ")
    } else {
        label.replace('\n', "")
    };
    Button::new(id)
        .ghost()
        .accessibility_label(accessible.clone())
        .tooltip(accessible)
        .w(px((width + 10.).max(40.)))
        .h(px(63.))
        .p_0()
        .rounded(px(3.))
        .child(
            div()
                .flex()
                .flex_col()
                .items_center()
                .justify_start()
                .pt(px(2.))
                .h_full()
                .gap(px(4.))
                .child(Icon::new(icon).size(px(29.)).text_color(rgb(tint)))
                .child(
                    div()
                        .text_center()
                        .text_size(px(10.5))
                        .line_height(px(12.))
                        .children(lines.map(|line| div().child(line))),
                ),
        )
}

pub(super) fn group(label: &'static str, content: impl IntoElement) -> Div {
    div()
        .h(px(RIBBON_HEIGHT))
        .flex_shrink_0()
        .px(px(6.))
        .border_r_1()
        .border_color(gpui::rgb(0x888888).opacity(0.27))
        .flex()
        .items_start()
        .child(joyoffice_ui::ribbon_group(
            div().child(content),
            label,
            label,
        ))
}

pub(super) fn row() -> Div {
    div().flex().items_center().gap(px(1.))
}

pub(super) fn col() -> Div {
    div().flex().flex_col().gap(px(4.))
}

use gpui_kit::component::tooltip::Tooltip;

fn theme_swatch_body(
    theme: &joydeck_model::Theme,
    locale: UiLocale,
    current: bool,
    p: Palette,
) -> Div {
    use joydeck_model::SchemeSlot as Slot;
    let color = |slot| {
        let c = theme.colors.get(slot);
        rgb(u32::from_be_bytes([0, c.r, c.g, c.b]))
    };
    let display_name = presentation_theme_name(locale, &theme.name);
    div()
        .w(px(88.))
        .h(px(50.))
        .rounded(px(3.))
        .overflow_hidden()
        .flex()
        .flex_col()
        .border_1()
        .border_color(rgb(if current { p.accent } else { p.border }))
        .bg(color(Slot::Lt1))
        .text_color(color(Slot::Dk1))
        .child(div().h(px(3.)).w_full().bg(color(Slot::Accent1)))
        .child(
            div()
                .flex_1()
                .px(px(6.))
                .py(px(3.))
                .flex()
                .flex_col()
                .justify_between()
                .child(
                    div()
                        .flex()
                        .items_baseline()
                        .justify_between()
                        .gap(px(2.))
                        .child(
                            div()
                                .text_size(px(15.))
                                .font_weight(FontWeight::SEMIBOLD)
                                .child("Aa"),
                        )
                        .child(
                            div()
                                .text_size(px(10.))
                                .truncate()
                                .max_w(px(52.))
                                .child(display_name),
                        ),
                )
                .child(
                    row().gap(px(2.)).children(
                        [
                            Slot::Accent1,
                            Slot::Accent2,
                            Slot::Accent3,
                            Slot::Accent4,
                            Slot::Accent5,
                            Slot::Accent6,
                        ]
                        .map(|slot| div().w(px(10.)).h(px(5.)).rounded(px(1.)).bg(color(slot))),
                    ),
                ),
        )
}

fn variant_swatch_body(
    scheme: &joydeck_model::ColorScheme,
    label: &str,
    current: bool,
    p: Palette,
) -> Div {
    use joydeck_model::SchemeSlot as Slot;
    let color = |slot| {
        let c = scheme.get(slot);
        rgb(u32::from_be_bytes([0, c.r, c.g, c.b]))
    };
    div()
        .w(px(108.))
        .h(px(56.))
        .px_2()
        .py(px(4.))
        .flex()
        .flex_col()
        .justify_between()
        .rounded(px(4.))
        .border_1()
        .border_color(rgb(if current { p.accent } else { p.border }))
        .bg(color(Slot::Lt1))
        .text_color(color(Slot::Dk1))
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .text_size(px(14.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child("Aa"),
                )
                .child(div().text_size(px(10.)).child(label.to_string())),
        )
        .child(
            row().gap(px(2.)).children(
                [
                    Slot::Accent1,
                    Slot::Accent2,
                    Slot::Accent3,
                    Slot::Accent4,
                    Slot::Accent5,
                    Slot::Accent6,
                ]
                .map(|slot| div().w(px(12.)).h(px(6.)).bg(color(slot))),
            ),
        )
}

impl Viewer {
    fn basic_button(&self, button: Button, edit: BasicEdit, cx: &mut Context<Self>) -> Button {
        let button = if let Some(reason) = self.basic_edit_unavailable(edit, cx) {
            button.tooltip(reason).disabled(true)
        } else {
            button
        };
        button.on_click(cx.listener(move |s, _, w, cx| s.run_basic_edit(edit, w, cx)))
    }

    pub(super) fn title_chrome(&self, cx: &mut Context<Self>) -> Div {
        let locale = self.locale;
        let p = self.palette();
        let dirty = self.editor.as_ref().is_some_and(|e| e.session.dirty());
        let name = self.document_name();
        let lang_view = cx.entity().downgrade();
        div()
            .h(px(33.))
            .flex_shrink_0()
            .relative()
            .flex()
            .items_center()
            .px_3()
            .when(cfg!(target_os = "macos"), |d| d.pl(px(82.)))
            .gap_1()
            .child(joyoffice_ui::product_logo(joyoffice_ui::Brand::Deck, 22.))
            .child(
                Button::new("file-home")
                    .ghost()
                    .xsmall()
                    .icon(I::House)
                    .accessibility_label(t(locale, "quick.file"))
                    .tooltip(t(locale, "quick.file"))
                    .on_click(cx.listener(|s, _, w, cx| {
                        s.open_file_page(w, cx);
                    })),
            )
            .child(
                Button::new("quick-save")
                    .ghost()
                    .xsmall()
                    .icon(I::Save)
                    .accessibility_label(t(locale, "quick.save"))
                    .tooltip(t(locale, "quick.save_tip"))
                    .disabled(self.save_unavailable().is_some())
                    .on_click(cx.listener(|s, _, w, cx| s.save(&Save, w, cx))),
            )
            .child(
                self.basic_button(
                    Button::new("quick-undo")
                        .ghost()
                        .xsmall()
                        .icon(I::Undo2)
                        .accessibility_label(t(locale, "quick.undo"))
                        .tooltip(t(locale, "quick.undo_tip")),
                    BasicEdit::Undo,
                    cx,
                ),
            )
            .child(
                self.basic_button(
                    Button::new("quick-redo")
                        .ghost()
                        .xsmall()
                        .icon(I::Redo2)
                        .accessibility_label(t(locale, "quick.redo"))
                        .tooltip(t(locale, "quick.redo_tip")),
                    BasicEdit::Redo,
                    cx,
                ),
            )
            .child(
                Button::new("quick-present")
                    .ghost()
                    .xsmall()
                    .icon(I::Presentation)
                    .accessibility_label(t(locale, "quick.present_start"))
                    .tooltip(t(locale, "quick.present_start_tip"))
                    .on_click(cx.listener(|s, _, _, cx| s.start_show(true, false, cx))),
            )
            .child(
                div().flex_1().min_w_0().flex().justify_center().child(
                    div()
                        .max_w_full()
                        .text_center()
                        .text_size(px(12.))
                        .text_ellipsis()
                        .child(format!("{name}{}", if dirty { " *" } else { "" })),
                ),
            )
            .child(
                Button::new("quick-find")
                    .ghost()
                    .xsmall()
                    .icon(I::Search)
                    .accessibility_label(t(locale, "quick.find"))
                    .tooltip(t(locale, "quick.find"))
                    .on_click(cx.listener(|s, _, w, cx| s.find_dialog(w, cx))),
            )
            .child(joyoffice_ui::language_menu(
                "quick-language-menu",
                locale,
                p,
                joyoffice_ui::language_header_trigger("quick-language", locale, p),
                move |next, w, cx| {
                    let _ = lang_view.update(cx, |s, cx| s.select_locale(next, w, cx));
                },
            ))
            .child(
                Button::new("quick-theme")
                    .ghost()
                    .xsmall()
                    .icon(if self.dark { I::Sun } else { I::Moon })
                    .accessibility_label(t(locale, "quick.theme"))
                    .tooltip(t(locale, "quick.theme"))
                    .on_click(cx.listener(|s, _, w, cx| {
                        s.select_theme(s.theme.with_dark(!s.dark), w, cx);
                    })),
            )
            .child(
                Button::new("present")
                    .label(t(locale, "quick.present"))
                    .icon(I::Play)
                    .h(px(26.))
                    .rounded(px(16.))
                    .bg(rgb(p.accent))
                    .text_color(rgb(p.on_accent))
                    .on_click(cx.listener(|s, _, _, cx| s.start_show(false, false, cx))),
            )
    }
    pub(super) fn editor_ribbon(&self, w: &Window, cx: &mut Context<Self>) -> Div {
        let locale = self.locale;
        let palette = self.palette();
        let selected = self.editor.as_ref().and_then(|e| {
            let id = *e.session.selection().shapes.first()?;
            e.session.model().slides.get(self.page)?.shape(id)
        });
        let contextual = RibbonTab::contextual(selected.map(|shape| &shape.kind));
        let active = self.ribbon.visible(self.backstage, contextual);
        let tabs = div()
            .id("ribbon-tabs")
            .role(gpui::Role::TabList)
            .aria_label(t(locale, "ribbon.aria"))
            .h(px(29.))
            .flex_shrink_0()
            .flex()
            .items_center()
            .overflow_x_scroll()
            .children(
                RibbonTab::STANDARD
                    .iter()
                    .chain(contextual)
                    .copied()
                    .map(|tab| self.ribbon_tab(tab, active == tab, palette.accent, cx)),
            )
            .child(div().flex_1())
            .child(
                Button::new("ribbon-comments")
                    .ghost()
                    .xsmall()
                    .icon(I::MessageSquare)
                    .label(t(locale, "ribbon.comments"))
                    .on_click(cx.listener(|s, _, w, cx| s.comments_dialog(w, cx))),
            );
        let ribbon = div().flex_shrink_0().child(tabs);
        if self.ribbon_collapsed || self.backstage {
            return ribbon;
        }
        let has_selection = self
            .editor
            .as_ref()
            .is_some_and(|e| !e.session.selection().shapes.is_empty());
        let bar = self.ribbon_commands(active, has_selection, cx);
        let viewport = div()
            .id("ribbon-scroll")
            .flex_1()
            .min_w_0()
            .flex()
            .items_start()
            .overflow_x_scroll()
            .track_scroll(&self.ribbon_scroll)
            .child(bar.flex_shrink_0());
        let narrow = f32::from(w.viewport_size().width) < RIBBON_OVERFLOW_WIDTH;
        ribbon.child(
            div()
                .id("ribbon-command-panel")
                .role(gpui::Role::Group)
                .aria_label(active.label(locale))
                .h(px(75.))
                .pt(px(5.))
                .pb(px(3.))
                .border_b_1()
                .border_color(rgb(palette.border))
                .bg(rgb(palette.chrome))
                .flex()
                .when(narrow, |d| d.child(self.ribbon_scroll_button(false, cx)))
                .child(viewport)
                .when(narrow, |d| d.child(self.ribbon_scroll_button(true, cx))),
        )
    }

    fn ribbon_commands(&self, tab: RibbonTab, selected: bool, cx: &mut Context<Self>) -> Div {
        let disabled = self.editor.is_none() || self.loading;
        match tab {
            RibbonTab::Home => self.ribbon_home(cx, disabled, selected),
            RibbonTab::Insert => self.ribbon_insert(cx),
            RibbonTab::Design => self.ribbon_design(cx, disabled),
            RibbonTab::Picture => self.ribbon_picture(cx, selected),
            RibbonTab::ChartDesign => self.ribbon_chart(cx, selected),
            RibbonTab::ChartFormat => self.ribbon_chart_format(cx),
            RibbonTab::TableDesign => self.table_controls(true, cx),
            RibbonTab::TableLayout => self.table_controls(false, cx),
            RibbonTab::Shape => self.ribbon_shape(cx),
            RibbonTab::Draw => self.ribbon_draw(cx),
            RibbonTab::Transitions => self.ribbon_transitions(cx),
            RibbonTab::Animations => self.ribbon_animations(cx),
            RibbonTab::Show => self.ribbon_show(cx),
            RibbonTab::Record => self.ribbon_record(cx),
            RibbonTab::Review => self.ribbon_review(cx),
            RibbonTab::View => self.ribbon_view(cx),
            RibbonTab::File => row().h(px(RIBBON_HEIGHT)).items_start(),
        }
    }

    fn ribbon_scroll_button(&self, forward: bool, cx: &mut Context<Self>) -> Button {
        let locale = self.locale;
        let (id, icon, label, step) = if forward {
            (
                "ribbon-right",
                I::ChevronRight,
                t(locale, "ribbon.scroll_right"),
                -RIBBON_SCROLL_STEP,
            )
        } else {
            (
                "ribbon-left",
                I::ChevronLeft,
                t(locale, "ribbon.scroll_left"),
                RIBBON_SCROLL_STEP,
            )
        };
        Button::new(id)
            .ghost()
            .icon(icon)
            .w(px(18.))
            .h(px(RIBBON_HEIGHT))
            .p_0()
            .accessibility_label(label)
            .on_click(cx.listener(move |s, _, _, cx| {
                let at = s.ribbon_scroll.offset().x + px(step);
                let limit = s.ribbon_scroll.max_offset().x.abs();
                s.ribbon_scroll
                    .set_offset(point(at.clamp(-limit, px(0.)), px(0.)));
                cx.notify();
            }))
    }

    fn ribbon_tab(
        &self,
        tab: RibbonTab,
        selected: bool,
        accent: u32,
        cx: &mut Context<Self>,
    ) -> Button {
        let label = tab.label(self.locale);
        Button::new(tab.id())
            .ghost()
            .xsmall()
            .h(px(28.))
            .p_0()
            .rounded_none()
            .flex_shrink_0()
            .accessibility_label(label)
            .role(gpui::Role::Tab)
            .selected(selected)
            .toggled(selected)
            .when(selected, |b| b.bg(gpui::transparent_black()))
            .child(
                div()
                    .mx(px(11.5))
                    .h(px(26.))
                    .flex()
                    .items_center()
                    .text_size(px(12.))
                    .when(selected, |d| d.font_weight(gpui::FontWeight::SEMIBOLD))
                    .border_b(px(2.))
                    .border_color(if selected {
                        rgb(accent).into()
                    } else {
                        gpui::transparent_black()
                    })
                    .child(label),
            )
            .on_click(cx.listener(move |s, event: &ClickEvent, w, cx| {
                match tab.press(selected, s.ribbon_collapsed, event.click_count() == 2) {
                    TabAction::Stay => return,
                    TabAction::SetFolded(folded) => s.ribbon_collapsed = folded,
                    TabAction::OpenFile => s.open_file_page(w, cx),
                    TabAction::Open(tab) => {
                        s.ribbon = tab;
                        s.ribbon_collapsed = false;
                        s.backstage = false;
                        s.ribbon_scroll.set_offset(point(px(0.), px(0.)));
                    }
                }
                cx.notify();
            }))
    }

    // Keep ribbon construction in bounded frames on the Windows default stack.
    #[inline(never)]
    fn ribbon_home(&self, cx: &mut Context<Self>, disabled: bool, selected: bool) -> Div {
        let locale = self.locale;
        let mut bar = row().h(px(RIBBON_HEIGHT)).items_start();
        bar = bar
            .child(self.home_clipboard(cx, disabled, selected))
            .child(self.home_slides(cx, disabled))
            .child(self.font_group(cx))
            .child(self.paragraph_group(cx))
            .child(self.home_drawing(cx, disabled))
            .child(self.home_arrange(cx))
            .child(group(
                t(locale, "home.group.extensions"),
                unavailable_row(
                    locale,
                    &[
                        ("home-addins", I::LayoutGrid, t(locale, "home.addins")),
                        ("home-designer", I::Zap, t(locale, "home.designer")),
                    ],
                ),
            ));
        bar
    }
    #[inline(never)]
    fn home_clipboard(&self, cx: &mut Context<Self>, disabled: bool, selected: bool) -> Div {
        let locale = self.locale;
        group(
            t(locale, "home.group.clipboard"),
            row()
                .child(
                    tool(
                        "paste",
                        I::ClipboardPaste,
                        t(locale, "home.paste"),
                        0x2e6fd8,
                    )
                    .disabled(disabled || self.composing())
                    .on_click(cx.listener(|s, _, w, cx| s.paste_grid(w, cx))),
                )
                .child(
                    col()
                        .child(
                            small("cut", I::Scissors, t(locale, "home.cut"))
                                .disabled(
                                    self.inline.is_none()
                                        && self
                                            .basic_edit_unavailable(BasicEdit::Delete, cx)
                                            .is_some(),
                                )
                                .on_click(cx.listener(|s, _, w, cx| {
                                    s.clipboard_copy(true, w, cx);
                                })),
                        )
                        .child(
                            small("copy", I::Copy, t(locale, "home.copy"))
                                .disabled(!selected)
                                .on_click(
                                    cx.listener(|s, _, w, cx| s.clipboard_copy(false, w, cx)),
                                ),
                        )
                        .child(small_unavailable(
                            locale,
                            "format-painter",
                            I::Paintbrush,
                            t(locale, "home.format_painter"),
                        )),
                ),
        )
    }
    #[inline(never)]
    fn home_slides(&self, cx: &mut Context<Self>, disabled: bool) -> Div {
        let locale = self.locale;
        let view = cx.entity().downgrade();
        group(
            t(locale, "home.group.slides"),
            row().child(self.new_slide_split(cx, "new-slide")).child(
                col()
                    .child(
                        small("layout", I::PanelsTopLeft, t(locale, "home.layout"))
                            .dropdown_caret(true)
                            .disabled(disabled)
                            .on_click(cx.listener(|s, _, w, cx| s.layout_gallery(w, cx))),
                    )
                    .child(small_unavailable(
                        locale,
                        "reset-slide",
                        I::RotateCw,
                        t(locale, "home.reset"),
                    ))
                    .child(
                        Popover::new("slide-section-menu")
                            .trigger(
                                small(
                                    "section-slides",
                                    I::GalleryVerticalEnd,
                                    t(locale, "home.section"),
                                )
                                .dropdown_caret(true)
                                .disabled(disabled),
                            )
                            .content(move |_, _, cx| {
                                let popup = cx.entity().downgrade();
                                let new_section = view.clone();
                                let move_slides = view.clone();
                                div()
                                    .flex()
                                    .flex_col()
                                    .child(
                                        small(
                                            "create-section",
                                            I::Plus,
                                            t(locale, "home.new_section"),
                                        )
                                        .on_click({
                                            let popup = popup.clone();
                                            move |_, w, cx| {
                                                let _ = popup.update(cx, |p, cx| p.dismiss(w, cx));
                                                let _ = new_section.update(cx, |s, cx| {
                                                    s.create_slide_section(w, cx)
                                                });
                                            }
                                        }),
                                    )
                                    .child(
                                        small(
                                            "move-slides",
                                            I::Move,
                                            t(locale, "home.move_slides"),
                                        )
                                        .on_click(
                                            move |_, w, cx| {
                                                let _ = popup.update(cx, |p, cx| p.dismiss(w, cx));
                                                let _ = move_slides
                                                    .update(cx, |s, cx| s.organize_dialog(w, cx));
                                            },
                                        ),
                                    )
                            }),
                    ),
            ),
        )
    }

    fn new_slide_split(&self, cx: &mut Context<Self>, id: &'static str) -> Div {
        let locale = self.locale;
        let view = cx.entity().downgrade();
        let can_duplicate = self
            .basic_edit_unavailable(BasicEdit::DuplicateSlides, cx)
            .is_none();
        let can_delete = self
            .basic_edit_unavailable(BasicEdit::DeleteSlides, cx)
            .is_none();
        row()
            .gap_0()
            .child(self.basic_button(
                tool(
                    id,
                    I::GalleryVerticalEnd,
                    t(locale, "home.new_slide"),
                    GREEN,
                ),
                BasicEdit::NewSlide,
                cx,
            ))
            .child(
                Popover::new((id, 0usize))
                    .trigger(
                        Button::new((id, 1usize))
                            .ghost()
                            .xsmall()
                            .p_0()
                            .w(px(14.))
                            .h(px(35.))
                            .icon(I::ChevronDown)
                            .accessibility_label(t(locale, "home.slide_options"))
                            .disabled(self.editor.is_none() || self.loading || self.composing()),
                    )
                    .content(move |_, _, cx| {
                        let popup = cx.entity().downgrade();
                        col().children(
                            [
                                (
                                    "duplicate-slide",
                                    I::Copy,
                                    t(locale, "home.duplicate_slide"),
                                    BasicEdit::DuplicateSlides,
                                    can_duplicate,
                                ),
                                (
                                    "delete-slide",
                                    I::Trash,
                                    t(locale, "home.delete_slide"),
                                    BasicEdit::DeleteSlides,
                                    can_delete,
                                ),
                            ]
                            .into_iter()
                            .map(|(id, icon, label, edit, enabled)| {
                                let view = view.clone();
                                let popup = popup.clone();
                                small(id, icon, label).disabled(!enabled).on_click(
                                    move |_, w, cx| {
                                        let _ = popup.update(cx, |p, cx| p.dismiss(w, cx));
                                        let _ =
                                            view.update(cx, |s, cx| s.run_basic_edit(edit, w, cx));
                                    },
                                )
                            }),
                        )
                    }),
            )
    }

    #[inline(never)]
    fn home_drawing(&self, cx: &mut Context<Self>, disabled: bool) -> Div {
        let locale = self.locale;
        group(
            t(locale, "home.group.drawing"),
            row()
                .child(
                    tool("picture", I::Image, t(locale, "home.picture"), 0x2ea36f)
                        .disabled(disabled)
                        .on_click(cx.listener(|s, _, w, cx| {
                            s.content_form(content_ui::Form::ImportPicture(false), w, cx)
                        })),
                )
                .child(
                    tool("shapes", I::Shapes, t(locale, "home.shapes"), 0x2e6fd8)
                        .disabled(disabled)
                        .on_click(cx.listener(|s, _, w, cx| s.shape_gallery(w, cx))),
                )
                .child(
                    tool(
                        "text-box",
                        I::TextCursorInput,
                        t(locale, "home.text_box"),
                        0x2e6fd8,
                    )
                    .disabled(disabled)
                    .on_click(cx.listener(|s, _, w, cx| s.insert_textbox(w, cx))),
                ),
        )
    }
    #[inline(never)]
    fn shape_outline_popover(
        &self,
        id: &'static str,
        trigger: Button,
        cx: &mut Context<Self>,
    ) -> Popover {
        let locale = self.locale;
        let view = cx.entity().downgrade();
        let p = self.palette();
        let colors: [(u32, &'static str); 8] = [
            (0x1f2937, t(locale, "swatch.dark_gray")),
            (0x2678d0, t(locale, "swatch.blue")),
            (0xd04020, t(locale, "swatch.red_orange")),
            (0x25805d, t(locale, "swatch.green")),
            (0xd99b26, t(locale, "swatch.gold")),
            (0x7c4dff, t(locale, "swatch.purple")),
            (0x6b7280, t(locale, "swatch.mid_gray")),
            (0xffffff, t(locale, "swatch.white")),
        ];
        Popover::new(id)
            .trigger(trigger.disabled(!self.can_edit_outline()))
            .content(move |_, _, cx| {
                let popup = cx.entity().downgrade();
                let color_row = row().gap_1().children(colors.iter().map(|(hex, tip)| {
                    let view = view.clone();
                    let popup = popup.clone();
                    let rgb_val = *hex;
                    Button::new(("outline-color", rgb_val))
                        .ghost()
                        .w(px(22.))
                        .h(px(22.))
                        .p_0()
                        .tooltip(*tip)
                        .child(
                            div()
                                .w(px(16.))
                                .h(px(16.))
                                .rounded(px(2.))
                                .border_1()
                                .border_color(rgb(p.border))
                                .bg(rgb(rgb_val)),
                        )
                        .on_click(move |_, w, cx| {
                            let _ = popup.update(cx, |pop, cx| pop.dismiss(w, cx));
                            let _ = view.update(cx, |s, cx| {
                                s.set_shape_line(Some(rgb_val), None, None, w, cx);
                            });
                        })
                }));
                let width_row = row().gap_1().children(
                    [
                        (0.75, "0.75pt"),
                        (1.5, "1.5pt"),
                        (2.25, "2.25pt"),
                        (3.0, "3pt"),
                        (4.5, "4.5pt"),
                    ]
                    .into_iter()
                    .enumerate()
                    .map(|(i, (pt, label))| {
                        let view = view.clone();
                        let popup = popup.clone();
                        Button::new(("outline-w", i))
                            .ghost()
                            .xsmall()
                            .h(px(20.))
                            .label(label)
                            .on_click(move |_, w, cx| {
                                let _ = popup.update(cx, |pop, cx| pop.dismiss(w, cx));
                                let _ = view.update(cx, |s, cx| {
                                    let current_rgb = s
                                        .current_slide()
                                        .and_then(|sl| {
                                            let id = *s
                                                .editor
                                                .as_ref()?
                                                .session
                                                .selection()
                                                .shapes
                                                .first()?;
                                            sl.shape(id)?.line.as_ref()?.fill.as_ref()
                                        })
                                        .and_then(|f| match f {
                                            joydeck_model::Fill::Solid { color } => {
                                                match color.base {
                                                    joydeck_model::style::ColorBase::Rgb {
                                                        rgb,
                                                    } => Some(
                                                        ((rgb.r as u32) << 16)
                                                            | ((rgb.g as u32) << 8)
                                                            | (rgb.b as u32),
                                                    ),
                                                    _ => None,
                                                }
                                            }
                                            _ => None,
                                        })
                                        .unwrap_or(0x2678d0);
                                    s.set_shape_line(Some(current_rgb), Some(pt), None, w, cx);
                                });
                            })
                    }),
                );
                let dash_row = row().gap_1().children(
                    [
                        ("solid", t(locale, "outline.dash_solid")),
                        ("dash", t(locale, "outline.dash_dash")),
                        ("dot", t(locale, "outline.dash_dot")),
                        ("dashDot", t(locale, "outline.dash_dash_dot")),
                    ]
                    .into_iter()
                    .enumerate()
                    .map(|(i, (preset, label))| {
                        let view = view.clone();
                        let popup = popup.clone();
                        Button::new(("outline-d", i))
                            .ghost()
                            .xsmall()
                            .h(px(20.))
                            .label(label)
                            .on_click(move |_, w, cx| {
                                let _ = popup.update(cx, |pop, cx| pop.dismiss(w, cx));
                                let _ = view.update(cx, |s, cx| {
                                    let current_rgb = s
                                        .current_slide()
                                        .and_then(|sl| {
                                            let id = *s
                                                .editor
                                                .as_ref()?
                                                .session
                                                .selection()
                                                .shapes
                                                .first()?;
                                            sl.shape(id)?.line.as_ref()?.fill.as_ref()
                                        })
                                        .and_then(|f| match f {
                                            joydeck_model::Fill::Solid { color } => {
                                                match color.base {
                                                    joydeck_model::style::ColorBase::Rgb {
                                                        rgb,
                                                    } => Some(
                                                        ((rgb.r as u32) << 16)
                                                            | ((rgb.g as u32) << 8)
                                                            | (rgb.b as u32),
                                                    ),
                                                    _ => None,
                                                }
                                            }
                                            _ => None,
                                        })
                                        .unwrap_or(0x2678d0);
                                    s.set_shape_line(
                                        Some(current_rgb),
                                        None,
                                        Some(preset.into()),
                                        w,
                                        cx,
                                    );
                                });
                            })
                    }),
                );
                let clear_view = view.clone();
                let clear_popup = popup.clone();
                let custom_view = view.clone();
                let custom_popup = popup.clone();
                col()
                    .gap_2()
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(p.muted))
                            .child(t(locale, "outline.color")),
                    )
                    .child(color_row)
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(p.muted))
                            .child(t(locale, "outline.weight")),
                    )
                    .child(width_row)
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(p.muted))
                            .child(t(locale, "outline.dashes")),
                    )
                    .child(dash_row)
                    .child(
                        row()
                            .gap_2()
                            .child(
                                small("outline-none", I::ImageOff, t(locale, "outline.none"))
                                    .on_click(move |_, w, cx| {
                                        let _ =
                                            clear_popup.update(cx, |pop, cx| pop.dismiss(w, cx));
                                        let _ = clear_view.update(cx, |s, cx| {
                                            s.set_shape_line(None, None, None, w, cx);
                                        });
                                    }),
                            )
                            .child(
                                small("outline-custom", I::PenLine, t(locale, "outline.custom"))
                                    .on_click(move |_, w, cx| {
                                        let _ =
                                            custom_popup.update(cx, |pop, cx| pop.dismiss(w, cx));
                                        let _ = custom_view.update(cx, |s, cx| {
                                            s.content_form(content_ui::Form::ShapeLine, w, cx);
                                        });
                                    }),
                            ),
                    )
            })
    }

    #[inline(never)]
    fn align_popover(&self, id: &'static str, cx: &mut Context<Self>) -> Popover {
        let locale = self.locale;
        let view = cx.entity().downgrade();
        let can_align = self.can_align();
        let can_dist = self.can_distribute();
        Popover::new(id)
            .trigger(
                tool(
                    "align-btn",
                    I::LayoutGrid,
                    t(locale, "align.button"),
                    0xa77436,
                )
                .dropdown_caret(true)
                .disabled(!can_align),
            )
            .content(move |_, _, cx| {
                let popup = cx.entity().downgrade();
                let aligns = [
                    (
                        "align-left",
                        I::TextAlignStart,
                        t(locale, "align.left"),
                        AlignEdge::Left,
                    ),
                    (
                        "align-center-h",
                        I::TextAlignCenter,
                        t(locale, "align.center_h"),
                        AlignEdge::CenterH,
                    ),
                    (
                        "align-right",
                        I::TextAlignEnd,
                        t(locale, "align.right"),
                        AlignEdge::Right,
                    ),
                    (
                        "align-top",
                        I::ArrowUpToLine,
                        t(locale, "align.top"),
                        AlignEdge::Top,
                    ),
                    (
                        "align-middle-v",
                        I::Columns2,
                        t(locale, "align.middle_v"),
                        AlignEdge::MiddleV,
                    ),
                    (
                        "align-bottom",
                        I::ArrowLeftToLine,
                        t(locale, "align.bottom"),
                        AlignEdge::Bottom,
                    ),
                ];
                let dists = [
                    (
                        "dist-h",
                        I::MoveHorizontal,
                        t(locale, "align.distribute_h"),
                        DistributeAxis::Horizontal,
                    ),
                    (
                        "dist-v",
                        I::GalleryVerticalEnd,
                        t(locale, "align.distribute_v"),
                        DistributeAxis::Vertical,
                    ),
                ];
                col()
                    .gap_1()
                    .children(aligns.into_iter().map(|(bid, icon, label, edge)| {
                        let view = view.clone();
                        let popup = popup.clone();
                        small(bid, icon, label)
                            .disabled(!can_align)
                            .on_click(move |_, w, cx| {
                                let _ = popup.update(cx, |p, cx| p.dismiss(w, cx));
                                let _ = view.update(cx, |s, cx| s.align_selected(edge, w, cx));
                            })
                    }))
                    .children(dists.into_iter().map(|(bid, icon, label, axis)| {
                        let view = view.clone();
                        let popup = popup.clone();
                        small(bid, icon, label)
                            .disabled(!can_dist)
                            .on_click(move |_, w, cx| {
                                let _ = popup.update(cx, |p, cx| p.dismiss(w, cx));
                                let _ = view.update(cx, |s, cx| s.distribute_selected(axis, w, cx));
                            })
                    }))
            })
    }

    #[inline(never)]
    fn group_popover(&self, id: &'static str, cx: &mut Context<Self>) -> Popover {
        let locale = self.locale;
        let view = cx.entity().downgrade();
        let can_group = self.can_group();
        let can_ungroup = self.can_ungroup();
        Popover::new(id)
            .trigger(
                tool("group-btn", I::Box, t(locale, "group.button"), 0xa77436)
                    .dropdown_caret(true)
                    .disabled(!can_group && !can_ungroup),
            )
            .content(move |_, _, cx| {
                let popup = cx.entity().downgrade();
                let gv = view.clone();
                let gp = popup.clone();
                let uv = view.clone();
                let up = popup.clone();
                col()
                    .gap_1()
                    .child(
                        small("group-objs", I::Box, t(locale, "group.button"))
                            .disabled(!can_group)
                            .on_click(move |_, w, cx| {
                                let _ = gp.update(cx, |p, cx| p.dismiss(w, cx));
                                let _ = gv.update(cx, |s, cx| s.group_selected(w, cx));
                            }),
                    )
                    .child(
                        small(
                            "ungroup-objs",
                            I::SquareSplitHorizontal,
                            t(locale, "group.ungroup"),
                        )
                        .disabled(!can_ungroup)
                        .on_click(move |_, w, cx| {
                            let _ = up.update(cx, |p, cx| p.dismiss(w, cx));
                            let _ = uv.update(cx, |s, cx| s.ungroup_selected(w, cx));
                        }),
                    )
            })
    }

    #[inline(never)]
    fn home_arrange(&self, cx: &mut Context<Self>) -> Div {
        let locale = self.locale;
        let view = cx.entity().downgrade();
        let can_group = self.can_group();
        let can_ungroup = self.can_ungroup();
        let can_align = self.can_align();
        let can_dist = self.can_distribute();
        let moves = [
            (
                "forward",
                I::BringToFront,
                t(locale, "order.bring_forward"),
                BasicEdit::BringForward,
            ),
            (
                "backward",
                I::SendToBack,
                t(locale, "order.send_backward"),
                BasicEdit::SendBackward,
            ),
            (
                "duplicate",
                I::CopyPlus,
                t(locale, "order.duplicate"),
                BasicEdit::Duplicate,
            ),
        ]
        .map(|(id, icon, label, edit)| {
            (
                id,
                icon,
                label,
                edit,
                self.basic_edit_unavailable(edit, cx).is_none(),
            )
        });
        group(
            t(locale, "home.group.arrange"),
            row()
                .child(
                    Popover::new("arrange-menu")
                        .trigger(
                            tool("arrange", I::Layers2, t(locale, "home.arrange"), 0xa77436)
                                .dropdown_caret(true),
                        )
                        .content(move |_, _, cx| {
                            let popup = cx.entity().downgrade();
                            let selection_view = view.clone();
                            let gv = view.clone();
                            let gp = popup.clone();
                            let uv = view.clone();
                            let up = popup.clone();
                            let aligns = [
                                (
                                    "home-align-l",
                                    I::TextAlignStart,
                                    t(locale, "align.left"),
                                    AlignEdge::Left,
                                ),
                                (
                                    "home-align-c",
                                    I::TextAlignCenter,
                                    t(locale, "align.center_h"),
                                    AlignEdge::CenterH,
                                ),
                                (
                                    "home-align-r",
                                    I::TextAlignEnd,
                                    t(locale, "align.right"),
                                    AlignEdge::Right,
                                ),
                                (
                                    "home-align-t",
                                    I::ArrowUpToLine,
                                    t(locale, "align.top"),
                                    AlignEdge::Top,
                                ),
                                (
                                    "home-align-m",
                                    I::Columns2,
                                    t(locale, "align.middle_v"),
                                    AlignEdge::MiddleV,
                                ),
                                (
                                    "home-align-b",
                                    I::ArrowLeftToLine,
                                    t(locale, "align.bottom"),
                                    AlignEdge::Bottom,
                                ),
                            ];
                            let dists = [
                                (
                                    "home-dist-h",
                                    I::MoveHorizontal,
                                    t(locale, "align.distribute_h"),
                                    DistributeAxis::Horizontal,
                                ),
                                (
                                    "home-dist-v",
                                    I::GalleryVerticalEnd,
                                    t(locale, "align.distribute_v"),
                                    DistributeAxis::Vertical,
                                ),
                            ];
                            col()
                                .gap_1()
                                .children(moves.into_iter().map(
                                    |(id, icon, label, edit, enabled)| {
                                        let view = view.clone();
                                        let popup = popup.clone();
                                        small(id, icon, label).disabled(!enabled).on_click(
                                            move |_, w, cx| {
                                                let _ = popup.update(cx, |p, cx| p.dismiss(w, cx));
                                                let _ = view.update(cx, |s, cx| {
                                                    s.run_basic_edit(edit, w, cx)
                                                });
                                            },
                                        )
                                    },
                                ))
                                .child(
                                    small("home-group", I::Box, t(locale, "group.button"))
                                        .disabled(!can_group)
                                        .on_click(move |_, w, cx| {
                                            let _ = gp.update(cx, |p, cx| p.dismiss(w, cx));
                                            let _ = gv.update(cx, |s, cx| s.group_selected(w, cx));
                                        }),
                                )
                                .child(
                                    small(
                                        "home-ungroup",
                                        I::SquareSplitHorizontal,
                                        t(locale, "group.ungroup"),
                                    )
                                    .disabled(!can_ungroup)
                                    .on_click(
                                        move |_, w, cx| {
                                            let _ = up.update(cx, |p, cx| p.dismiss(w, cx));
                                            let _ =
                                                uv.update(cx, |s, cx| s.ungroup_selected(w, cx));
                                        },
                                    ),
                                )
                                .children(aligns.into_iter().map(|(id, icon, label, edge)| {
                                    let view = view.clone();
                                    let popup = popup.clone();
                                    small(id, icon, label).disabled(!can_align).on_click(
                                        move |_, w, cx| {
                                            let _ = popup.update(cx, |p, cx| p.dismiss(w, cx));
                                            let _ = view
                                                .update(cx, |s, cx| s.align_selected(edge, w, cx));
                                        },
                                    )
                                }))
                                .children(dists.into_iter().map(|(id, icon, label, axis)| {
                                    let view = view.clone();
                                    let popup = popup.clone();
                                    small(id, icon, label).disabled(!can_dist).on_click(
                                        move |_, w, cx| {
                                            let _ = popup.update(cx, |p, cx| p.dismiss(w, cx));
                                            let _ = view.update(cx, |s, cx| {
                                                s.distribute_selected(axis, w, cx)
                                            });
                                        },
                                    )
                                }))
                                .child(
                                    small(
                                        "selection",
                                        I::PanelRight,
                                        t(locale, "home.selection_pane"),
                                    )
                                    .on_click(
                                        move |_, w, cx| {
                                            let _ = popup.update(cx, |p, cx| p.dismiss(w, cx));
                                            let _ = selection_view.update(cx, |s, cx| {
                                                s.inspector = !s.inspector;
                                                cx.notify();
                                            });
                                        },
                                    ),
                                )
                        }),
                )
                .child(unavailable(
                    locale,
                    "quick-shape-style",
                    I::Brush,
                    t(locale, "home.quick_styles"),
                ))
                .child(
                    col()
                        .pt(px(4.))
                        .child(
                            small(
                                "home-shape-fill",
                                I::PaintBucket,
                                t(locale, "home.shape_fill"),
                            )
                            .disabled(
                                self.editor
                                    .as_ref()
                                    .is_none_or(|e| e.session.selection().shapes.is_empty())
                                    || self.loading
                                    || self.composing(),
                            )
                            .on_click(cx.listener(|s, _, w, cx| {
                                s.content_form(content_ui::Form::ShapeFill, w, cx)
                            })),
                        )
                        .child(self.shape_outline_popover(
                            "home-shape-outline-pop",
                            small(
                                "home-shape-outline",
                                I::PenLine,
                                t(locale, "home.shape_outline"),
                            ),
                            cx,
                        )),
                ),
        )
    }

    // Keep ribbon construction in bounded frames on the Windows default stack.
    #[inline(never)]
    fn ribbon_insert(&self, cx: &mut Context<Self>) -> Div {
        let locale = self.locale;
        let disabled = self.editor.is_none() || self.loading || self.composing();
        let mut bar = row().h(px(RIBBON_HEIGHT)).items_start();
        bar = bar
            .child(group(
                t(locale, "home.group.slides"),
                self.new_slide_split(cx, "insert-slide"),
            ))
            .child(group(
                t(locale, "insert.group.table"),
                tool("insert-table", I::Table, t(locale, "insert.table"), BLUE)
                    .dropdown_caret(true)
                    .disabled(disabled)
                    .on_click(cx.listener(|s, _, w, cx| s.insert_table_dialog(w, cx))),
            ))
            .child(group(
                t(locale, "insert.group.images"),
                row()
                    .child(
                        tool(
                            "insert-picture",
                            I::Image,
                            t(locale, "insert.picture"),
                            BLUE,
                        )
                        .disabled(disabled)
                        .on_click(cx.listener(|s, _, w, cx| {
                            s.content_form(content_ui::Form::ImportPicture(false), w, cx)
                        })),
                    )
                    .child(unavailable(
                        locale,
                        "insert-screenshot",
                        I::Scan,
                        t(locale, "insert.screenshot"),
                    )),
            ))
            .child(group(
                t(locale, "insert.camera"),
                unavailable(locale, "insert-cameo", I::Camera, "Cameo"),
            ))
            .child(group(
                t(locale, "insert.group.illustrations"),
                row()
                    .child(
                        tool("insert-shapes", I::Shapes, t(locale, "insert.shapes"), BLUE)
                            .dropdown_caret(true)
                            .disabled(disabled)
                            .on_click(cx.listener(|s, _, w, cx| s.shape_gallery(w, cx))),
                    )
                    .child(unavailable_row(
                        locale,
                        &[
                            ("insert-icons", I::Shapes, t(locale, "insert.icons")),
                            ("insert-3d", I::Box, t(locale, "insert.models_3d")),
                            ("insert-smartart", I::Network, "SmartArt"),
                            ("insert-chart", I::ChartColumn, t(locale, "insert.chart")),
                        ],
                    )),
            ))
            .child(group(
                t(locale, "insert.group.links"),
                row()
                    .child(unavailable(
                        locale,
                        "insert-zoom",
                        I::Scan,
                        t(locale, "insert.zoom"),
                    ))
                    .child(self.text_link_controls(cx))
                    .child(unavailable(
                        locale,
                        "insert-action",
                        I::MousePointer,
                        t(locale, "insert.action"),
                    )),
            ))
            .child(group(
                t(locale, "insert.group.comments"),
                tool(
                    "insert-comment",
                    I::MessageSquarePlus,
                    t(locale, "insert.comment"),
                    GREEN,
                )
                .disabled(disabled)
                .on_click(cx.listener(|s, _, w, cx| s.add_comment(w, cx))),
            ))
            .child(self.insert_text_group(disabled, cx))
            .child(group(
                t(locale, "insert.group.symbols"),
                unavailable_row(
                    locale,
                    &[
                        ("insert-equation", I::Sigma, t(locale, "insert.equation")),
                        ("insert-symbol", I::Omega, t(locale, "insert.symbol")),
                    ],
                ),
            ))
            .child(group(
                t(locale, "insert.group.media"),
                unavailable_row(
                    locale,
                    &[
                        ("insert-video", I::Video, t(locale, "insert.video")),
                        ("insert-audio", I::Volume2, t(locale, "insert.audio")),
                    ],
                ),
            ));
        bar
    }

    #[inline(never)]
    fn insert_text_group(&self, disabled: bool, cx: &mut Context<Self>) -> Div {
        let locale = self.locale;
        group(
            t(locale, "insert.group.text"),
            row()
                .child(
                    tool(
                        "insert-textbox",
                        I::TextCursorInput,
                        t(locale, "insert.textbox"),
                        BLUE,
                    )
                    .disabled(disabled)
                    .on_click(cx.listener(|s, _, w, cx| s.insert_textbox(w, cx))),
                )
                .child(self.footer_control(cx))
                .child(unavailable(
                    locale,
                    "insert-wordart",
                    I::Type,
                    t(locale, "insert.wordart"),
                ))
                .children(self.text_field_controls(cx))
                .child(unavailable(
                    locale,
                    "insert-object",
                    I::FileBox,
                    t(locale, "insert.object"),
                )),
        )
    }

    // Keep ribbon construction in bounded frames on the Windows default stack.
    #[inline(never)]
    fn ribbon_design(&self, cx: &mut Context<Self>, disabled: bool) -> Div {
        let locale = self.locale;
        let editor = self.editor.as_ref();
        let master = editor.and_then(|e| {
            let model = e.session.model();
            model
                .master_for(model.slides.get(self.page)?)
                .map(|(master, _)| master)
        });
        let p = self.palette();
        let builtin = joydeck_model::theme::builtin_themes();
        let current_name = master.map(|m| m.theme.name.clone());
        let current_is_builtin = current_name
            .as_deref()
            .is_some_and(|name| builtin.iter().any(|t| t.name == name));
        let mut theme_cards: Vec<AnyElement> = Vec::new();
        let mut idx = 0usize;
        if let Some(m) = master
            && !current_is_builtin
        {
            let name = presentation_theme_name(locale, &m.theme.name);
            let tip = tf(locale, "design.current_theme_tip", &[("name", &name)]);
            theme_cards.push(
                div()
                    .id(("theme-preview", idx))
                    .tooltip(move |w, cx| Tooltip::new(tip.clone()).build(w, cx))
                    .child(theme_swatch_body(&m.theme, locale, true, p))
                    .into_any_element(),
            );
            idx += 1;
        }
        for theme in &builtin {
            let is_current = current_name.as_deref() == Some(theme.name.as_str());
            let apply_name = theme.name.clone();
            let display_name = presentation_theme_name(locale, &apply_name);
            let hint = presentation_theme_hint(locale, &apply_name);
            let tip = if is_current {
                tf(
                    locale,
                    "design.theme_reapply_tip",
                    &[("name", &display_name), ("hint", hint)],
                )
            } else {
                tf(
                    locale,
                    "design.theme_apply_tip",
                    &[("name", &display_name), ("hint", hint)],
                )
            };
            theme_cards.push(
                div()
                    .id(("theme-preview", idx))
                    .flex_shrink_0()
                    .when(!disabled, |d| d.cursor_pointer())
                    .tooltip(move |w, cx| Tooltip::new(tip.clone()).build(w, cx))
                    .when(!disabled, |d| {
                        d.on_click(cx.listener(move |s, _, w, cx| {
                            s.dispatch(
                                Command::Content {
                                    action: joydeck_engine::ContentAction::ApplyTheme {
                                        name: apply_name.clone(),
                                    },
                                },
                                w,
                                cx,
                            );
                        }))
                    })
                    .child(theme_swatch_body(theme, locale, is_current, p))
                    .into_any_element(),
            );
            idx += 1;
        }
        let themes_strip = gallery(self.palette())
            .id("design-themes-gallery")
            .max_w(px(580.))
            .overflow_x_scroll()
            .children(theme_cards);
        let all_themes = builtin.clone();
        let all_current = current_name.clone();
        let themes_view = cx.entity().downgrade();
        let all_themes_btn = Popover::new("design-themes-all")
            .trigger(
                tool(
                    "design-themes-more",
                    I::LayoutGrid,
                    t(locale, "design.all_themes"),
                    BLUE,
                )
                .dropdown_caret(true)
                .disabled(disabled),
            )
            .content(move |_, _, _| {
                let mut grid = div().flex().flex_wrap().gap_2().w(px(484.));
                for (ti, theme) in all_themes.iter().enumerate() {
                    let is_curr = all_current.as_deref() == Some(theme.name.as_str());
                    let display_name = presentation_theme_name(locale, &theme.name);
                    let hint = presentation_theme_hint(locale, &theme.name);
                    let tip = format!("{display_name} — {hint}");
                    let view = themes_view.clone();
                    grid = grid.child(
                        div()
                            .id(("theme-all-card", ti))
                            .w(px(90.))
                            .flex()
                            .flex_col()
                            .gap(px(2.))
                            .cursor_pointer()
                            .tooltip(move |w, cx| Tooltip::new(tip.clone()).build(w, cx))
                            .on_click({
                                let apply_name = theme.name.clone();
                                move |_, w, cx| {
                                    let _ = view.update(cx, |s, cx| {
                                        s.dispatch(
                                            Command::Content {
                                                action: joydeck_engine::ContentAction::ApplyTheme {
                                                    name: apply_name.clone(),
                                                },
                                            },
                                            w,
                                            cx,
                                        );
                                    });
                                }
                            })
                            .child(theme_swatch_body(theme, locale, is_curr, p)),
                    );
                }
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .text_xs()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(rgb(p.muted))
                            .child(t(locale, "design.all_themes_heading")),
                    )
                    .child(grid)
            });
        let themes = row().gap_1().child(themes_strip).child(all_themes_btn);

        let variant_schemes = master
            .map(|m| joydeck_model::theme::theme_variants(&m.theme))
            .unwrap_or_default();
        let current_colors = master.map(|m| m.theme.colors.clone());
        let view = cx.entity().downgrade();
        let variant_btn = Popover::new("design-variants")
            .trigger(
                tool(
                    "design-variants-trigger",
                    I::Brush,
                    t(locale, "design.variants"),
                    BLUE,
                )
                .dropdown_caret(true)
                .disabled(disabled || variant_schemes.is_empty()),
            )
            .content(move |_, _, _| {
                let labels = [
                    t(locale, "design.variant_default"),
                    t(locale, "design.variant_2"),
                    t(locale, "design.variant_3"),
                    t(locale, "design.variant_invert"),
                ];
                let fallback_label = t(locale, "design.variants");
                let mut grid = div().flex().flex_wrap().gap_2().w(px(236.));
                for (vi, scheme) in variant_schemes.iter().enumerate() {
                    let is_curr = current_colors.as_ref() == Some(scheme);
                    let label = labels.get(vi).copied().unwrap_or(fallback_label);
                    let view = view.clone();
                    grid = grid.child(
                        div()
                            .id(("theme-variant-card", vi))
                            .cursor_pointer()
                            .on_click(move |_, w, cx| {
                                let _ = view.update(cx, |s, cx| {
                                    s.dispatch(
                                        Command::Content {
                                            action:
                                                joydeck_engine::ContentAction::ApplyThemeVariant {
                                                    index: vi,
                                                },
                                        },
                                        w,
                                        cx,
                                    );
                                });
                            })
                            .child(variant_swatch_body(scheme, label, is_curr, p)),
                    );
                }
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .text_xs()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(rgb(p.muted))
                            .child(t(locale, "design.variants_heading")),
                    )
                    .child(grid)
            });

        let mut variants = row().child(variant_btn);
        for (id, label, icon, form) in [
            (
                "theme-colors",
                t(locale, "design.colors"),
                I::Palette,
                content_ui::Form::Accent,
            ),
            (
                "theme-fonts",
                t(locale, "design.fonts"),
                I::Type,
                content_ui::Form::Fonts,
            ),
            (
                "slide-background",
                t(locale, "design.background_styles"),
                I::PaintBucket,
                content_ui::Form::Background,
            ),
            (
                "master-background",
                t(locale, "design.master_background"),
                I::PanelsTopLeft,
                content_ui::Form::MasterBackground,
            ),
        ] {
            variants = variants.child(
                tool(id, icon, label, BLUE)
                    .disabled(disabled)
                    .on_click(cx.listener(move |s, _, w, cx| s.content_form(form, w, cx))),
            );
        }
        row()
            .h(px(RIBBON_HEIGHT))
            .child(group(t(locale, "design.group.themes"), themes))
            .child(group(t(locale, "design.group.variants"), variants))
            .child(group(
                t(locale, "design.group.customize"),
                row()
                    .child(
                        tool(
                            "design-layout",
                            I::PanelsTopLeft,
                            t(locale, "design.layout"),
                            BLUE,
                        )
                        .dropdown_caret(true)
                        .disabled(disabled)
                        .on_click(cx.listener(|s, _, w, cx| s.layout_gallery(w, cx))),
                    )
                    .child(
                        tool(
                            "slide-size",
                            I::RectangleHorizontal,
                            t(locale, "design.slide_size"),
                            BLUE,
                        )
                        .disabled(disabled)
                        .on_click(
                            cx.listener(|s, _, w, cx| {
                                s.content_form(content_ui::Form::Size, w, cx)
                            }),
                        ),
                    ),
            ))
            .child(group(
                t(locale, "design.group.designer"),
                unavailable(locale, "designer", I::Zap, t(locale, "design.designer_btn")),
            ))
    }

    // Keep ribbon construction in bounded frames on the Windows default stack.
    #[inline(never)]
    fn ribbon_picture(&self, cx: &mut Context<Self>, selected: bool) -> Div {
        let locale = self.locale;
        let adjustment = row()
            .child(unavailable(
                locale,
                "picture-remove-bg",
                I::ImageOff,
                t(locale, "pic.remove_bg"),
            ))
            .child(
                tool(
                    "picture-corrections",
                    I::Sun,
                    t(locale, "pic.corrections"),
                    0xae6f30,
                )
                .disabled(!selected)
                .on_click(
                    cx.listener(|s, _, w, cx| s.content_form(content_ui::Form::Picture, w, cx)),
                ),
            )
            .child(unavailable(
                locale,
                "picture-color",
                I::Image,
                t(locale, "pic.color"),
            ))
            .child(unavailable(
                locale,
                "picture-effects",
                I::Sparkles,
                t(locale, "pic.artistic_effects"),
            ))
            .child(
                tool(
                    "picture-alpha",
                    I::Blend,
                    t(locale, "pic.transparency"),
                    BLUE,
                )
                .disabled(!selected)
                .on_click(
                    cx.listener(|s, _, w, cx| s.content_form(content_ui::Form::Picture, w, cx)),
                ),
            )
            .child(
                col()
                    .child(small_unavailable(
                        locale,
                        "picture-compress",
                        I::Minimize2,
                        t(locale, "pic.compress"),
                    ))
                    .child(
                        small("replace-picture", I::ImagePlus, t(locale, "pic.change"))
                            .disabled(!selected)
                            .on_click(cx.listener(|s, _, w, cx| {
                                s.content_form(content_ui::Form::ImportPicture(true), w, cx)
                            })),
                    )
                    .child(small_unavailable(
                        locale,
                        "picture-reset",
                        I::RotateCw,
                        t(locale, "pic.reset"),
                    )),
            );
        row()
            .h(px(RIBBON_HEIGHT))
            .child(group(t(locale, "pic.group.adjust"), adjustment))
            .child(group(
                t(locale, "pic.group.styles"),
                unavailable_row(
                    locale,
                    &[
                        ("picture-style", I::Image, t(locale, "pic.group.styles")),
                        ("picture-border", I::PenLine, t(locale, "pic.border")),
                        ("picture-layout", I::PanelsTopLeft, t(locale, "pic.layout")),
                    ],
                ),
            ))
            .child(self.object_alt_text(cx))
            .child(self.object_arrange(cx))
            .child(group(
                t(locale, "pic.group.size"),
                row()
                    .child(
                        tool("crop-picture", I::Crop, t(locale, "pic.crop"), BLUE)
                            .disabled(!selected)
                            .on_click(cx.listener(|s, _, w, cx| {
                                s.content_form(content_ui::Form::Picture, w, cx)
                            })),
                    )
                    .child(
                        tool("picture-size", I::Move, t(locale, "pic.size_pos"), BLUE)
                            .disabled(!selected)
                            .on_click(cx.listener(|s, _, w, cx| s.geometry_dialog(w, cx))),
                    ),
            ))
    }

    fn object_alt_text(&self, cx: &mut Context<Self>) -> Div {
        let locale = self.locale;
        group(
            t(locale, "review.group.accessibility"),
            tool(
                "object-alt-text",
                I::FilePenLine,
                t(locale, "review.alt_text_long"),
                BLUE,
            )
            .on_click(cx.listener(|s, _, w, cx| s.accessibility_dialog(w, cx))),
        )
    }

    fn object_arrange(&self, cx: &mut Context<Self>) -> Div {
        let locale = self.locale;
        group(
            t(locale, "home.group.arrange"),
            row()
                .child(self.basic_button(
                    tool(
                        "object-forward",
                        I::BringToFront,
                        t(locale, "order.bring_forward_alt"),
                        0xa77436,
                    ),
                    BasicEdit::BringForward,
                    cx,
                ))
                .child(self.basic_button(
                    tool(
                        "object-backward",
                        I::SendToBack,
                        t(locale, "order.send_backward_alt"),
                        0xa77436,
                    ),
                    BasicEdit::SendBackward,
                    cx,
                ))
                .child(self.align_popover("object-align-pop", cx))
                .child(self.group_popover("object-group-pop", cx))
                .child(
                    tool(
                        "object-selection-pane",
                        I::PanelRight,
                        t(locale, "view.selection_pane"),
                        BLUE,
                    )
                    .toggled(self.inspector)
                    .selected(self.inspector)
                    .on_click(cx.listener(|s, _, _, cx| {
                        s.inspector = !s.inspector;
                        cx.notify();
                    })),
                ),
        )
    }

    // Keep ribbon construction in bounded frames on the Windows default stack.
    #[inline(never)]
    fn ribbon_chart(&self, cx: &mut Context<Self>, selected: bool) -> Div {
        use joydeck_model::chart::ChartType;
        let locale = self.locale;
        let current = self.editor.as_ref().and_then(|e| {
            let id = e.session.selection().shapes.first()?;
            let shape = self.current_slide()?.shape(*id)?;
            match &shape.kind {
                joydeck_model::ShapeKind::Chart(chart) => Some(&chart.kind),
                _ => None,
            }
        });
        let mut kinds = gallery(self.palette());
        for (id, label, icon, kind) in [
            (
                "chart-column",
                t(locale, "chart.column"),
                I::ChartColumn,
                ChartType::Column,
            ),
            (
                "chart-line",
                t(locale, "chart.line"),
                I::ChartLine,
                ChartType::Line,
            ),
            (
                "chart-pie",
                t(locale, "chart.pie"),
                I::ChartPie,
                ChartType::Pie,
            ),
        ] {
            kinds = kinds.child(
                tool(id, icon, label, BLUE)
                    .w(px(80.))
                    .h(px(52.))
                    .selected(current == Some(&kind))
                    .disabled(!selected)
                    .on_click(cx.listener(move |s, _, w, cx| {
                        s.dispatch(
                            Command::Content {
                                action: joydeck_engine::ContentAction::ChartType {
                                    chart_type: kind,
                                },
                            },
                            w,
                            cx,
                        )
                    })),
            );
        }
        row()
            .h(px(RIBBON_HEIGHT))
            .child(group(
                t(locale, "chart.group.layouts"),
                row()
                    .child(
                        tool("chart-title", I::Type, t(locale, "chart.title_btn"), BLUE)
                            .disabled(!selected)
                            .on_click(cx.listener(|s, _, w, cx| {
                                s.content_form(content_ui::Form::ChartTitle, w, cx)
                            })),
                    )
                    .child(unavailable(
                        locale,
                        "chart-layout",
                        I::PanelsTopLeft,
                        t(locale, "chart.quick_layout"),
                    )),
            ))
            .child(group(
                t(locale, "chart.group.styles"),
                unavailable(
                    locale,
                    "chart-colors",
                    I::Palette,
                    t(locale, "chart.change_colors"),
                ),
            ))
            .child(group(t(locale, "chart.group.change_type"), kinds))
            .child(group(
                t(locale, "chart.group.data"),
                row()
                    .child(unavailable(
                        locale,
                        "chart-select-data",
                        I::Table,
                        t(locale, "chart.select_data"),
                    ))
                    .child(
                        tool("chart-data", I::Table, t(locale, "chart.edit_data"), BLUE)
                            .disabled(!selected)
                            .on_click(cx.listener(|s, _, w, cx| {
                                s.content_form(content_ui::Form::ChartValue, w, cx)
                            })),
                    )
                    .child(unavailable(
                        locale,
                        "chart-refresh-data",
                        I::RotateCw,
                        t(locale, "chart.refresh_data"),
                    )),
            ))
    }

    #[inline(never)]
    fn ribbon_chart_format(&self, cx: &mut Context<Self>) -> Div {
        let locale = self.locale;
        row()
            .h(px(RIBBON_HEIGHT))
            .child(group(
                t(locale, "chart.group.current_selection"),
                unavailable(
                    locale,
                    "chart-selection",
                    I::ChartColumn,
                    t(locale, "chart.area"),
                ),
            ))
            .child(group(
                t(locale, "chart.group.shape_styles"),
                unavailable_row(
                    locale,
                    &[
                        (
                            "chart-shape-style",
                            I::Brush,
                            t(locale, "home.quick_styles"),
                        ),
                        (
                            "chart-shape-outline",
                            I::PenLine,
                            t(locale, "home.shape_outline"),
                        ),
                    ],
                ),
            ))
            .child(self.object_arrange(cx))
            .child(group(
                t(locale, "pic.group.size"),
                tool("chart-size", I::Move, t(locale, "pic.size_pos"), BLUE)
                    .on_click(cx.listener(|s, _, w, cx| s.geometry_dialog(w, cx))),
            ))
    }
    // Keep ribbon construction in bounded frames on the Windows default stack.
    #[inline(never)]
    fn ribbon_shape(&self, cx: &mut Context<Self>) -> Div {
        let locale = self.locale;
        let mut bar = row().h(px(RIBBON_HEIGHT)).items_start();
        bar = bar
            .child(group(
                t(locale, "shape.group.insert"),
                row()
                    .child(
                        tool("format-shapes", I::Shapes, t(locale, "home.shapes"), BLUE)
                            .dropdown_caret(true)
                            .on_click(cx.listener(|s, _, w, cx| s.shape_gallery(w, cx))),
                    )
                    .child(
                        tool(
                            "format-textbox",
                            I::TextCursorInput,
                            t(locale, "home.text_box"),
                            BLUE,
                        )
                        .on_click(cx.listener(|s, _, w, cx| s.insert_textbox(w, cx))),
                    ),
            ))
            .child(group(
                t(locale, "shape.group.styles"),
                row()
                    .child(
                        tool(
                            "shape-fill",
                            I::PaintBucket,
                            t(locale, "home.shape_fill"),
                            0xf26b1d,
                        )
                        .on_click(cx.listener(|s, _, w, cx| {
                            s.content_form(content_ui::Form::ShapeFill, w, cx)
                        })),
                    )
                    .child(
                        tool(
                            "shape-gradient",
                            I::Blend,
                            t(locale, "shape.gradient_fill"),
                            0x8a5cd8,
                        )
                        .on_click(cx.listener(|s, _, w, cx| {
                            s.content_form(content_ui::Form::ShapeGradient, w, cx)
                        })),
                    )
                    .child(
                        self.shape_outline_popover(
                            "shape-outline-pop",
                            tool(
                                "shape-outline",
                                I::PenLine,
                                t(locale, "home.shape_outline"),
                                0x2678d0,
                            )
                            .dropdown_caret(true),
                            cx,
                        ),
                    ),
            ))
            .child(self.font_group(cx))
            .child(self.paragraph_group(cx))
            .child(self.text_layout_group(cx))
            .child(self.object_alt_text(cx))
            .child(self.object_arrange(cx))
            .child(group(
                t(locale, "shape.group.size_pos"),
                row()
                    .child(
                        tool(
                            "geometry",
                            I::Move,
                            t(locale, "shape.size_and_pos"),
                            0x2e6fd8,
                        )
                        .on_click(cx.listener(|s, _, w, cx| s.geometry_dialog(w, cx))),
                    )
                    .child(
                        tool(
                            "rotate",
                            I::RotateCw,
                            t(locale, "shape.rotate_right"),
                            0x2e6fd8,
                        )
                        .on_click(cx.listener(|s, _, w, cx| {
                            s.dispatch(
                                Command::Transform {
                                    dx: 0.,
                                    dy: 0.,
                                    dw: 0.,
                                    dh: 0.,
                                    rotation: 90.,
                                },
                                w,
                                cx,
                            )
                        })),
                    ),
            ));
        bar
    }
    // Keep ribbon construction in bounded frames on the Windows default stack.
    #[inline(never)]
    fn ribbon_show(&self, cx: &mut Context<Self>) -> Div {
        let locale = self.locale;
        let disabled = self.editor.is_none() || self.loading || self.composing();
        let hidden = self.current_slide().is_some_and(|slide| slide.hidden);
        let show = self.editor.as_ref().map(|e| &e.session.model().show);
        row()
            .h(px(RIBBON_HEIGHT))
            .child(group(
                t(locale, "show.group.start"),
                row()
                    .child(
                        tool(
                            "show-start",
                            I::Presentation,
                            t(locale, "show.from_beginning"),
                            GREEN,
                        )
                        .disabled(disabled)
                        .on_click(cx.listener(|s, _, _, cx| s.start_show(true, false, cx))),
                    )
                    .child(
                        tool(
                            "show-current",
                            I::MonitorPlay,
                            t(locale, "show.from_current"),
                            GREEN,
                        )
                        .disabled(disabled)
                        .on_click(cx.listener(|s, _, _, cx| s.start_show(false, false, cx))),
                    )
                    .child(
                        tool(
                            "show-presenter",
                            I::PanelsTopLeft,
                            t(locale, "show.presenter_view"),
                            BLUE,
                        )
                        .disabled(disabled)
                        .on_click(cx.listener(|s, _, _, cx| s.start_show(false, true, cx))),
                    )
                    .child(unavailable(
                        locale,
                        "custom-show",
                        I::PanelsTopLeft,
                        t(locale, "show.custom_show"),
                    )),
            ))
            .child(group(
                t(locale, "show.group.setup"),
                row()
                    .child(unavailable(
                        locale,
                        "show-setup",
                        I::Settings,
                        t(locale, "show.setup"),
                    ))
                    .child(
                        tool("hide-slides", I::EyeOff, t(locale, "show.hide_slide"), BLUE)
                            .toggled(hidden)
                            .selected(hidden)
                            .disabled(disabled)
                            .on_click(cx.listener(|s, _, w, cx| s.toggle_hidden_slides(w, cx))),
                    ),
            ))
            .child(group(
                t(locale, "show.group.timing"),
                row()
                    .child(unavailable(
                        locale,
                        "show-rehearse",
                        I::Timer,
                        t(locale, "show.rehearse"),
                    ))
                    .child(unavailable(
                        locale,
                        "show-record",
                        I::Video,
                        t(locale, "show.record"),
                    ))
                    .child(ribbon_checks(
                        locale,
                        &[
                            (t(locale, "show.keep_updated"), false),
                            (
                                t(locale, "show.play_narrations"),
                                show.is_some_and(|settings| !settings.without_narration),
                            ),
                        ],
                    ))
                    .child(ribbon_checks(
                        locale,
                        &[
                            (
                                t(locale, "show.use_timings"),
                                show.is_some_and(|settings| settings.use_timings),
                            ),
                            (
                                t(locale, "show.show_media_controls"),
                                show.is_some_and(|settings| settings.show_media_controls),
                            ),
                        ],
                    )),
            ))
    }

    // Keep ribbon construction in bounded frames on the Windows default stack.
    #[inline(never)]
    fn ribbon_review(&self, cx: &mut Context<Self>) -> Div {
        let locale = self.locale;
        let disabled = self.editor.is_none() || self.loading || self.composing();
        let mut bar = row().h(px(RIBBON_HEIGHT));
        bar = bar
            .child(group(
                t(locale, "review.group.proofing"),
                unavailable_row(
                    locale,
                    &[
                        (
                            "review-spelling",
                            I::SpellCheck,
                            t(locale, "review.spelling"),
                        ),
                        (
                            "review-thesaurus",
                            I::BookOpen,
                            t(locale, "review.thesaurus"),
                        ),
                    ],
                ),
            ))
            .child(group(
                t(locale, "review.group.accessibility"),
                tool(
                    "accessibility",
                    I::Accessibility,
                    t(locale, "review.alt_text"),
                    BLUE,
                )
                .disabled(
                    disabled
                        || self
                            .editor
                            .as_ref()
                            .is_none_or(|e| e.session.selection().shapes.is_empty()),
                )
                .on_click(cx.listener(|s, _, w, cx| s.accessibility_dialog(w, cx))),
            ))
            .child(group(
                t(locale, "review.group.language"),
                unavailable_row(
                    locale,
                    &[
                        (
                            "review-translate",
                            I::Languages,
                            t(locale, "review.translate"),
                        ),
                        (
                            "review-language",
                            I::Languages,
                            t(locale, "review.language"),
                        ),
                    ],
                ),
            ))
            .child(group(
                t(locale, "review.group.compare"),
                tool(
                    "compare-documents",
                    I::FileCheck,
                    t(locale, "review.compare_docs"),
                    BLUE,
                )
                .disabled(disabled)
                .on_click(cx.listener(|s, _, w, cx| s.compare_file(w, cx))),
            ))
            .child(group(
                t(locale, "review.group.comments"),
                row()
                    .child(
                        tool(
                            "add-comment",
                            I::MessageSquarePlus,
                            t(locale, "review.new_comment"),
                            GREEN,
                        )
                        .disabled(disabled)
                        .on_click(cx.listener(|s, _, w, cx| s.add_comment(w, cx))),
                    )
                    .child(unavailable_row(
                        locale,
                        &[
                            (
                                "review-delete-comment",
                                I::Trash,
                                t(locale, "common.delete"),
                            ),
                            (
                                "review-prev-comment",
                                I::ArrowLeft,
                                t(locale, "review.prev_comment"),
                            ),
                            (
                                "review-next-comment",
                                I::ArrowRight,
                                t(locale, "review.next_comment"),
                            ),
                        ],
                    ))
                    .child(
                        tool(
                            "comments",
                            I::MessageSquare,
                            t(locale, "review.show_comments"),
                            BLUE,
                        )
                        .disabled(disabled)
                        .on_click(cx.listener(|s, _, w, cx| s.comments_dialog(w, cx))),
                    ),
            ))
            .child(self.review_document_tools(cx));
        bar
    }

    #[inline(never)]
    fn review_document_tools(&self, cx: &mut Context<Self>) -> Div {
        let locale = self.locale;
        group(
            t(locale, "review.group.document"),
            row()
                .child(
                    tool(
                        "compatibility",
                        I::FileCheck,
                        t(locale, "review.compatibility"),
                        GREEN,
                    )
                    .on_click(cx.listener(|s, _, w, cx| s.report(&Report, w, cx))),
                )
                .child(
                    tool(
                        "metadata",
                        I::FilePenLine,
                        t(locale, "review.properties"),
                        BLUE,
                    )
                    .on_click(
                        cx.listener(|s, _, w, cx| {
                            s.content_form(content_ui::Form::Metadata, w, cx)
                        }),
                    ),
                )
                .child(unavailable(
                    locale,
                    "review-hide-ink",
                    I::PenLine,
                    t(locale, "review.hide_ink"),
                )),
        )
    }

    // Keep ribbon construction in bounded frames on the Windows default stack.
    #[inline(never)]
    fn ribbon_view(&self, cx: &mut Context<Self>) -> Div {
        let locale = self.locale;
        row()
            .h(px(RIBBON_HEIGHT))
            .child(group(
                t(locale, "view.group.presentation"),
                row()
                    .child(
                        tool("normal", I::PanelsTopLeft, t(locale, "view.normal"), BLUE)
                            .toggled(!self.sorter)
                            .selected(!self.sorter)
                            .on_click(cx.listener(|s, _, _, cx| {
                                s.sorter = false;
                                cx.notify();
                            })),
                    )
                    .child(unavailable(
                        locale,
                        "outline-view",
                        I::ListTree,
                        t(locale, "view.outline"),
                    ))
                    .child(
                        tool("sorter", I::LayoutGrid, t(locale, "view.sorter"), BLUE)
                            .toggled(self.sorter)
                            .selected(self.sorter)
                            .on_click(cx.listener(|s, _, _, cx| {
                                s.sorter = true;
                                cx.notify();
                            })),
                    )
                    .child(
                        tool(
                            "review-notes",
                            I::NotebookPen,
                            t(locale, "view.notes_page"),
                            BLUE,
                        )
                        .on_click(cx.listener(|s, _, w, cx| s.edit_notes(w, cx))),
                    )
                    .child(unavailable(
                        locale,
                        "reading-view",
                        I::BookOpen,
                        t(locale, "view.reading"),
                    )),
            ))
            .child(group(
                t(locale, "view.group.master"),
                unavailable_row(
                    locale,
                    &[
                        (
                            "slide-master",
                            I::PanelsTopLeft,
                            t(locale, "view.slide_master"),
                        ),
                        (
                            "handout-master",
                            I::LayoutGrid,
                            t(locale, "view.handout_master"),
                        ),
                        (
                            "notes-master",
                            I::NotebookText,
                            t(locale, "view.notes_master"),
                        ),
                    ],
                ),
            ))
            .child(group(
                t(locale, "view.group.show"),
                row()
                    .child(ribbon_checks(
                        locale,
                        &[
                            (t(locale, "view.ruler"), false),
                            (t(locale, "view.gridlines"), false),
                            (t(locale, "view.guides"), false),
                        ],
                    ))
                    .child(
                        tool("notes-view", I::NotebookPen, t(locale, "view.notes"), BLUE)
                            .toggled(self.notes_visible)
                            .selected(self.notes_visible)
                            .on_click(cx.listener(|s, _, _, cx| {
                                s.notes_visible = !s.notes_visible;
                                cx.notify();
                            })),
                    ),
            ))
            .child(group(
                t(locale, "view.group.zoom"),
                row()
                    .child(
                        tool("view-zoom", I::Search, t(locale, "view.zoom"), BLUE)
                            .on_click(cx.listener(|s, _, w, cx| s.ribbon_zoom_dialog(w, cx))),
                    )
                    .child(
                        tool("view-fit", I::Scan, t(locale, "view.fit_window"), BLUE)
                            .on_click(cx.listener(|s, _, w, cx| s.fit(&Fit, w, cx))),
                    ),
            ))
            .child(self.view_window_tools(cx))
    }

    fn ribbon_zoom_dialog(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        let locale = self.locale;
        self.fields_dialog(
            t(locale, "view.zoom"),
            vec![(
                t(locale, "view.zoom_field"),
                format!("{:.0}", self.scale(w) * 100.),
            )],
            w,
            cx,
            |s, values, _, cx| {
                let value = values[0]
                    .trim()
                    .parse::<f32>()
                    .map_err(|_| s.t("view.zoom_err_nan"))?;
                if !(10. ..=400.).contains(&value) {
                    return Err(s.t("view.zoom_err_range").into());
                }
                s.zoom = Some(value / 100.);
                cx.notify();
                Ok(())
            },
        );
    }

    #[inline(never)]
    fn view_window_tools(&self, cx: &mut Context<Self>) -> Div {
        let locale = self.locale;
        group(
            t(locale, "view.group.window"),
            row()
                .child(
                    tool(
                        "shared-view",
                        I::PanelsTopLeft,
                        t(locale, "view.new_window"),
                        BLUE,
                    )
                    .disabled(self.editor.is_none() || self.loading)
                    .on_click(cx.listener(|s, _, _, cx| s.new_shared_view(cx))),
                )
                .child(
                    tool(
                        "properties",
                        I::PanelRight,
                        t(locale, "view.selection_pane"),
                        BLUE,
                    )
                    .toggled(self.inspector)
                    .selected(self.inspector)
                    .on_click(cx.listener(|s, _, _, cx| {
                        s.inspector = !s.inspector;
                        cx.notify();
                    })),
                )
                .child(
                    tool(
                        "render-report",
                        I::Activity,
                        t(locale, "view.render_report"),
                        GREEN,
                    )
                    .on_click(cx.listener(|s, _, w, cx| s.scene_report(w, cx))),
                )
                .child(
                    tool("render-mode", I::Layers, "CPU / GPUI", 0x8a5cd8).on_click(cx.listener(
                        |s, _, w, cx| {
                            s.cpu_view = !s.cpu_view;
                            s.gpu_cache.clear(w);
                            cx.notify();
                        },
                    )),
                ),
        )
    }
    pub(super) fn status_chrome(&self, scale: f32, cx: &mut Context<Self>) -> Div {
        let locale = self.locale;
        let p = self.palette();
        let count = self.deck.as_ref().map_or(0, |d| d.info.slides.len());
        let current_str = (if count == 0 { 0 } else { self.page + 1 }).to_string();
        let total_str = count.to_string();
        let status_lang_view = cx.entity().downgrade();
        div()
            .h(px(26.))
            .flex_shrink_0()
            .border_t_1()
            .border_color(rgb(p.border))
            .px_3()
            .flex()
            .items_center()
            .gap_2()
            .text_size(px(11.))
            .child(tf(
                locale,
                "status.slide_of",
                &[("current", &current_str), ("total", &total_str)],
            ))
            .child(div().w(px(1.)).h(px(12.)).bg(rgb(p.border)))
            .child(joyoffice_ui::language_menu(
                "status-language-menu",
                locale,
                p,
                joyoffice_ui::language_status_trigger("status-language", locale, p),
                move |next, w, cx| {
                    let _ = status_lang_view.update(cx, |s, cx| s.select_locale(next, w, cx));
                },
            ))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .text_ellipsis()
                    .text_color(rgb(p.muted))
                    .child(if self.loading {
                        t(locale, "status.opening").into()
                    } else if self.is_saving() {
                        t(locale, "status.saving").into()
                    } else {
                        self.message.clone()
                    }),
            )
            .when(self.loading, |bar| {
                bar.child(
                    Button::new("cancel-open")
                        .ghost()
                        .xsmall()
                        .label(t(locale, "status.cancel_open"))
                        .on_click(cx.listener(|s, _, _, cx| s.cancel_read(cx))),
                )
            })
            .child(
                Button::new("status-notes")
                    .ghost()
                    .xsmall()
                    .icon(I::NotebookPen)
                    .label(t(locale, "status.notes"))
                    .on_click(cx.listener(|s, _, _, cx| {
                        s.notes_visible = !s.notes_visible;
                        cx.notify();
                    })),
            )
            .child(
                Button::new("status-normal")
                    .ghost()
                    .xsmall()
                    .icon(I::PanelsTopLeft)
                    .accessibility_label(t(locale, "status.view_normal"))
                    .on_click(cx.listener(|s, _, _, cx| {
                        s.sorter = false;
                        cx.notify();
                    })),
            )
            .child(
                Button::new("status-sorter")
                    .ghost()
                    .xsmall()
                    .icon(I::LayoutGrid)
                    .accessibility_label(t(locale, "status.view_sorter"))
                    .on_click(cx.listener(|s, _, _, cx| {
                        s.sorter = true;
                        cx.notify();
                    })),
            )
            .child(
                Button::new("status-show")
                    .ghost()
                    .xsmall()
                    .icon(I::Presentation)
                    .accessibility_label(t(locale, "status.present_current"))
                    .on_click(cx.listener(|s, _, _, cx| s.start_show(false, false, cx))),
            )
            .child(
                Button::new("status-minus")
                    .ghost()
                    .xsmall()
                    .icon(I::Minus)
                    .accessibility_label(t(locale, "status.zoom_out"))
                    .on_click(cx.listener(|s, _, w, cx| s.zoom_out(&ZoomOut, w, cx))),
            )
            .child(
                div()
                    .id("zoom-track")
                    .w(px(100.))
                    .h(px(20.))
                    .relative()
                    .cursor_pointer()
                    .child(
                        canvas(
                            cx.processor(|s, bounds, _, _| s.zoom_bounds = bounds),
                            |_, _, _, _| {},
                        )
                        .size_full(),
                    )
                    .child(
                        div()
                            .absolute()
                            .left_0()
                            .right_0()
                            .top(px(10.))
                            .h(px(1.))
                            .bg(rgb(p.muted)),
                    )
                    .child(
                        div()
                            .absolute()
                            .left(px((scale / 4.).clamp(0., 1.) * 96.))
                            .top(px(5.))
                            .w(px(4.))
                            .h(px(11.))
                            .bg(rgb(p.text)),
                    )
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|s, e: &MouseDownEvent, _, cx| {
                            let mouse = f32::from(e.position.x);
                            let left = f32::from(s.zoom_bounds.left());
                            let width = f32::from(s.zoom_bounds.size.width).max(1.);
                            s.zoom = Some(((mouse - left) / width * 4.).clamp(0.1, 4.));
                            cx.notify();
                        }),
                    ),
            )
            .child(
                Button::new("status-plus")
                    .ghost()
                    .xsmall()
                    .icon(I::Plus)
                    .accessibility_label(t(locale, "status.zoom_in"))
                    .on_click(cx.listener(|s, _, w, cx| s.zoom_in(&ZoomIn, w, cx))),
            )
            .child(
                div()
                    .w(px(40.))
                    .text_center()
                    .child(format!("{:.0}%", scale * 100.)),
            )
            .child(
                Button::new("status-fit")
                    .ghost()
                    .xsmall()
                    .icon(I::Scan)
                    .accessibility_label(t(locale, "status.fit_window"))
                    .tooltip(t(locale, "status.fit_window_tip"))
                    .on_click(cx.listener(|s, _, w, cx| s.fit(&Fit, w, cx))),
            )
    }
    pub(super) fn start_show(&mut self, first: bool, presenter: bool, cx: &mut Context<Self>) {
        if self.composing() {
            return;
        }
        self.end_inline(cx);
        if let Some(e) = &self.editor
            && let Err(error) = slideshow::open(
                e.session.model_snapshot(),
                if first { None } else { Some(self.page) },
                true,
                presenter,
                cx,
            )
        {
            self.message = error;
            cx.notify();
        }
    }
}
