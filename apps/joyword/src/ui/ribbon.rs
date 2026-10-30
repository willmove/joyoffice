use crate::chrome_ui::drawn_icon;
use crate::ui::controls;
use crate::ui::theme;
use crate::{JoyWord, PopupKind, SidePanel};
use gpui::{Context, KeyDownEvent, Role, Window, div, prelude::*, px, rgb};
use joyword_shell::chrome::{RibbonControl, RibbonTab};
use joyword_shell::command::*;
use std::rc::Rc;

pub(crate) fn ribbon_density(width: f32) -> u8 {
    if width < 760.0 {
        2
    } else if width < 1180.0 {
        1
    } else {
        0
    }
}

impl JoyWord {
    pub(crate) fn word_ribbon(&self, window: &Window, cx: &mut Context<Self>) -> impl IntoElement {
        let width = f32::from(window.viewport_size().width);
        let density = ribbon_density(width);
        div()
            .flex()
            .flex_col()
            .bg(rgb(theme::RIBBON))
            .border_b_1()
            .border_color(rgb(theme::BORDER))
            .child(self.word_ribbon_tabs(cx))
            .child(
                div()
                    .id("ribbon-row")
                    .role(Role::Toolbar)
                    .h(px(88.0))
                    .px_2()
                    .py_1()
                    .child(if density == 2 {
                        self.overflow_ribbon(cx)
                    } else {
                        match self.ribbon_tab {
                            RibbonTab::Start => self.start_ribbon(density == 1, cx),
                            RibbonTab::Insert => self.insert_ribbon(cx),
                            RibbonTab::Layout => self.layout_ribbon(cx),
                            RibbonTab::Review => self.review_ribbon(cx),
                            RibbonTab::View => self.view_ribbon(cx),
                        }
                    }),
            )
    }

    fn word_ribbon_tabs(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("ribbon-tabs")
            .role(Role::TabList)
            .flex()
            .items_center()
            .h(px(theme::TAB_HEIGHT))
            .gap_1()
            .px_2()
            .children(RibbonTab::ALL.into_iter().map(|tab| {
                let selected = self.ribbon_tab == tab;
                div()
                    .id(tab.label_key())
                    .accessibility_id(tab.label_key())
                    .role(Role::Tab)
                    .aria_label(self.label(tab.label_key()))
                    .aria_selected(selected)
                    .key_context(controls::CONTROL_CONTEXT)
                    .focusable()
                    .px_3()
                    .py_1()
                    .cursor_pointer()
                    .rounded_sm()
                    .border_b_2()
                    .border_color(if selected {
                        rgb(theme::ACCENT)
                    } else {
                        gpui::transparent_black().into()
                    })
                    .text_color(rgb(if selected { theme::ACCENT } else { theme::TEXT }))
                    .hover(|style| style.bg(rgb(theme::HOVER)))
                    .focus(|style| style.bg(rgb(theme::SELECTED)))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.ribbon_tab = tab;
                        this.popup = None;
                        cx.notify();
                    }))
                    .on_key_down(cx.listener(move |this, event: &KeyDownEvent, _, cx| {
                        if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                            this.ribbon_tab = tab;
                            cx.stop_propagation();
                            cx.notify();
                        }
                    }))
                    .child(self.label(tab.label_key()))
            }))
    }

    fn start_ribbon(&self, compact: bool, cx: &mut Context<Self>) -> gpui::AnyElement {
        let (family, size, _, _, _, _) = self.font_prefill();
        let family = if family.is_empty() {
            self.label("ui.font_family")
        } else {
            family
        };
        let size = if size.is_empty() {
            "12".to_owned()
        } else {
            size
        };
        div()
            .flex()
            .items_stretch()
            .h_full()
            .child(self.group(
                "ribbon.group.clipboard",
                vec![
                    self.large_command_button(self.control("start.paste"), cx),
                    div()
                        .flex()
                        .flex_col()
                        .child(self.command_button(self.control("start.cut"), false, cx))
                        .child(self.command_button(self.control("start.copy"), false, cx))
                        .child(self.command_button(self.control("start.format_copy"), false, cx))
                        .into_any_element(),
                ],
                cx,
            ))
            .child(self.group(
                "ribbon.group.font",
                vec![
                    div()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .child(
                            div()
                                .flex()
                                .gap_1()
                                .child(self.dropdown(
                                    "font-family",
                                    family,
                                    PopupKind::FontFamily,
                                    150.0,
                                    cx,
                                ))
                                .child(self.dropdown(
                                    "font-size",
                                    size,
                                    PopupKind::FontSize,
                                    54.0,
                                    cx,
                                ))
                                .child(self.dropdown(
                                    "font-color",
                                    "A ▾".to_owned(),
                                    PopupKind::FontColor,
                                    42.0,
                                    cx,
                                )),
                        )
                        .child(
                            div()
                                .flex()
                                .gap_1()
                                .child(self.command_button(self.control("start.bold"), true, cx))
                                .child(self.command_button(self.control("start.italic"), true, cx))
                                .child(
                                    self.command_button(self.control("start.underline"), true, cx),
                                )
                                .child(self.command_button(self.control("start.font"), true, cx)),
                        )
                        .into_any_element(),
                ],
                cx,
            ))
            .child(self.group(
                "ribbon.group.paragraph",
                vec![
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(
                                div()
                                    .flex()
                                    .gap_1()
                                    .child(self.command_button(
                                        self.control("start.list_bullet"),
                                        true,
                                        cx,
                                    ))
                                    .child(self.command_button(
                                        self.control("start.list_level_1"),
                                        true,
                                        cx,
                                    ))
                                    .child(self.command_button(
                                        self.control("start.list_level_2"),
                                        true,
                                        cx,
                                    ))
                                    .child(self.command_button(
                                        self.control("start.list_remove"),
                                        true,
                                        cx,
                                    )),
                            )
                            .child(
                                div()
                                    .flex()
                                    .gap_1()
                                    .child(self.command_button(
                                        self.control("start.align_left"),
                                        true,
                                        cx,
                                    ))
                                    .child(self.command_button(
                                        self.control("start.align"),
                                        true,
                                        cx,
                                    ))
                                    .child(self.command_button(
                                        self.control("start.align_right"),
                                        true,
                                        cx,
                                    ))
                                    .child(self.command_button(
                                        self.control("start.justify"),
                                        true,
                                        cx,
                                    ))
                                    .child(self.command_button(
                                        self.control("start.paragraph"),
                                        true,
                                        cx,
                                    )),
                            )
                            .into_any_element(),
                    ],
                cx,
            ))
            .children((!compact).then(|| self.style_gallery(cx)))
            .child(self.group(
                "ribbon.group.editing",
                vec![
                    self.find_panel_button("start.find_next", false, cx),
                    self.find_panel_button("start.replace", true, cx),
                ],
                cx,
            ))
            .children(compact.then(|| self.more_button(cx)))
            .into_any_element()
    }

    fn insert_ribbon(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        div()
            .flex()
            .h_full()
            .child(self.group(
                "ribbon.group.pages",
                vec![self.command_button(self.control("insert.page_break"), false, cx)],
                cx,
            ))
            .child(self.group(
                "ribbon.group.tables",
                vec![self.dropdown(
                    "insert-table",
                    self.label("command.table.insert"),
                    PopupKind::TableGrid,
                    110.0,
                    cx,
                )],
                cx,
            ))
            .child(self.group(
                "ribbon.group.illustrations",
                vec![self.command_button(self.control("insert.picture"), false, cx)],
                cx,
            ))
            .into_any_element()
    }

    fn layout_ribbon(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        div()
            .flex()
            .h_full()
            .child(self.group(
                "ribbon.group.page",
                vec![
                    self.dropdown(
                        "page-size",
                        self.label("command.page.size"),
                        PopupKind::PageSize,
                        120.0,
                        cx,
                    ),
                    self.dropdown(
                        "page-margins",
                        self.label("command.page.margins"),
                        PopupKind::Margins,
                        110.0,
                        cx,
                    ),
                    self.dropdown(
                        "page-orientation",
                        self.label("command.page.orientation"),
                        PopupKind::Orientation,
                        110.0,
                        cx,
                    ),
                ],
                cx,
            ))
            .child(self.group(
                "ribbon.group.pages",
                vec![
                    self.command_button(self.control("layout.edit_header"), true, cx),
                    self.command_button(self.control("layout.edit_footer"), true, cx),
                    self.command_button(self.control("layout.edit_body"), true, cx),
                ],
                cx,
            ))
            .into_any_element()
    }

    fn review_ribbon(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        self.generic_ribbon(cx)
    }

    fn view_ribbon(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        self.generic_ribbon(cx)
    }

    fn overflow_ribbon(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        div()
            .flex()
            .items_center()
            .gap_1()
            .h_full()
            .children(
                ["start.bold", "start.italic", "start.underline"]
                    .into_iter()
                    .map(|id| self.command_button(self.control(id), true, cx)),
            )
            .child(self.more_button(cx))
            .into_any_element()
    }

    fn generic_ribbon(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let controls: Vec<_> = joyword_shell::chrome::RIBBON
            .iter()
            .filter(|control| control.tab == self.ribbon_tab)
            .collect();
        let groups = joyword_shell::chrome::groups(&controls);
        div()
            .flex()
            .h_full()
            .children(groups.into_iter().map(|(name, controls)| {
                self.group(
                    name,
                    controls
                        .into_iter()
                        .map(|control| self.command_button(Some(*control), true, cx))
                        .collect(),
                    cx,
                )
            }))
            .into_any_element()
    }

    fn group(
        &self,
        name: &'static str,
        controls: Vec<gpui::AnyElement>,
        _cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        div()
            .id(name)
            .flex()
            .flex_col()
            .h_full()
            .px_2()
            .border_r_1()
            .border_color(rgb(theme::BORDER_LIGHT))
            .child(
                div()
                    .flex_1()
                    .flex()
                    .items_center()
                    .gap_2()
                    .children(controls),
            )
            .child(
                div()
                    .flex()
                    .justify_center()
                    .text_xs()
                    .text_color(rgb(theme::TEXT_MUTED))
                    .child(self.label(name)),
            )
            .into_any_element()
    }

    fn control(&self, id: &str) -> Option<RibbonControl> {
        joyword_shell::chrome::RIBBON
            .iter()
            .find(|control| control.id == id)
            .copied()
    }

    fn command_button(
        &self,
        control: Option<RibbonControl>,
        compact: bool,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        self.command_button_with_size(control, compact, false, cx)
    }

    fn large_command_button(
        &self,
        control: Option<RibbonControl>,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        self.command_button_with_size(control, false, true, cx)
    }

    fn command_button_with_size(
        &self,
        control: Option<RibbonControl>,
        compact: bool,
        large: bool,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let Some(control) = control else {
            return div().into_any_element();
        };
        let enabled = self.ribbon_enabled(control.command);
        let pressed = enabled && self.ribbon_pressed(control.command);
        let ink = if enabled {
            theme::TEXT
        } else {
            theme::TEXT_DISABLED
        };
        let label = self.label(control.label);
        let tooltip = label.clone();
        let button = div()
            .id(control.id)
            .accessibility_id(control.id)
            .role(Role::Button)
            .aria_label(label.clone())
            .key_context(controls::CONTROL_CONTEXT)
            .when(
                matches!(
                    control.command,
                    EDIT_BOLD
                        | EDIT_ITALIC
                        | EDIT_UNDERLINE
                        | EDIT_ALIGN_LEFT
                        | EDIT_ALIGN_CENTER
                        | EDIT_ALIGN_RIGHT
                        | EDIT_JUSTIFY
                        | VIEW_FORMATTING_MARKS
                        | VIEW_HIDDEN_TEXT
                        | VIEW_FIELD_CODES
                        | REVIEW_MARKUP
                        | REVIEW_FINAL
                        | REVIEW_ORIGINAL
                        | REVIEW_SIMPLE
                        | REVIEW_TRACK
                ),
                |element| element.aria_toggled(pressed.into()),
            )
            .a11y_synthetic_children(move |builder| {
                if !enabled {
                    builder.parent_node().set_disabled();
                }
            })
            .when(enabled, |element| element.focusable())
            .cursor_pointer()
            .flex()
            .items_center()
            .justify_center()
            .gap_1()
            .px_1()
            .rounded_sm()
            .border_1()
            .border_color(if pressed {
                rgb(theme::ACCENT)
            } else {
                gpui::transparent_black().into()
            })
            .bg(if pressed {
                rgb(theme::SELECTED)
            } else {
                gpui::transparent_black().into()
            })
            .text_color(rgb(ink))
            .hover(|style| style.bg(rgb(theme::HOVER)))
            .active(|style| style.bg(rgb(theme::PRESSED)))
            .focus(|style| style.border_color(rgb(theme::ACCENT)))
            .on_click(cx.listener(move |this, _, window, cx| {
                if this.ribbon_enabled(control.command) {
                    this.run_ribbon(control, window, cx);
                }
            }))
            .on_key_down(cx.listener(move |this, event: &KeyDownEvent, window, cx| {
                if matches!(event.keystroke.key.as_str(), "enter" | "space")
                    && this.ribbon_enabled(control.command)
                {
                    this.run_ribbon(control, window, cx);
                    cx.stop_propagation();
                }
            }))
            .tooltip(move |_, cx| {
                cx.new(|_| crate::ui::ribbon::RibbonTooltip(tooltip.clone()))
                    .into()
            })
            .child(drawn_icon(control.icon, ink))
            .children((!compact).then(|| div().text_xs().child(label)));
        if large {
            button.flex_col().h_full().w(px(58.0)).into_any_element()
        } else {
            button.h(px(theme::CONTROL_HEIGHT)).into_any_element()
        }
    }

    fn dropdown(
        &self,
        id: &'static str,
        label: String,
        kind: PopupKind,
        width: f32,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        self.popup_trigger(id, label.clone(), kind, cx)
            .cursor_pointer()
            .flex()
            .items_center()
            .justify_between()
            .h(px(theme::CONTROL_HEIGHT))
            .w(px(width))
            .px_2()
            .rounded_sm()
            .border_1()
            .border_color(rgb(theme::BORDER))
            .bg(rgb(theme::WHITE))
            .text_sm()
            .hover(|style| style.border_color(rgb(theme::ACCENT)))
            .focus(|style| style.border_color(rgb(theme::ACCENT)))
            .child(label)
            .child("▾")
            .into_any_element()
    }

    fn style_gallery(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let rows = self
            .editor
            .as_ref()
            .map(|editor| editor.style_rows())
            .unwrap_or_default();
        let entity = cx.entity();
        self.group(
            "ribbon.group.styles",
            vec![
                div()
                    .flex()
                    .gap_1()
                    .children(rows.into_iter().take(6).map(|row| {
                        let id = row.id.clone();
                        div()
                            .id(SharedStringStyle::id(&row.id))
                            .cursor_pointer()
                            .flex()
                            .items_center()
                            .justify_center()
                            .w(px(92.0))
                            .h(px(52.0))
                            .px_2()
                            .border_1()
                            .border_color(rgb(theme::BORDER))
                            .bg(rgb(theme::WHITE))
                            .hover(|style| style.border_color(rgb(theme::ACCENT)))
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.run_with(
                                    STYLE_APPLY,
                                    &[("style", ArgValue::Text(id.clone()))],
                                    window,
                                    cx,
                                );
                            }))
                            .child(div().text_sm().child(row.name))
                            .into_any_element()
                    }))
                    .into_any_element(),
                controls::button(
                    "open-styles",
                    self.label("style.panel"),
                    self.label("style.panel"),
                    true,
                    matches!(self.side_panel, Some(SidePanel::Styles)),
                    true,
                    Rc::new(move |_, cx| {
                        entity.update(cx, |this, cx| {
                            this.side_panel = Some(SidePanel::Styles);
                            cx.notify();
                        });
                    }),
                ),
            ],
            cx,
        )
    }

    fn find_panel_button(
        &self,
        id: &'static str,
        replace: bool,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let label = self.label(if replace {
            "command.find.replace"
        } else {
            "find.query"
        });
        controls::named_control(id, Role::Button, label.clone())
            .focusable()
            .cursor_pointer()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap_1()
            .h_full()
            .w(px(58.0))
            .rounded_sm()
            .hover(|style| style.bg(rgb(theme::HOVER)))
            .on_click(cx.listener(move |this, _, window, cx| {
                this.show_find_panel(replace, window, cx);
                cx.notify();
            }))
            .on_key_down(cx.listener(move |this, event: &KeyDownEvent, window, cx| {
                if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                    this.show_find_panel(replace, window, cx);
                    cx.stop_propagation();
                    cx.notify();
                }
            }))
            .child(drawn_icon(
                if replace { "replace" } else { "find" },
                theme::TEXT,
            ))
            .child(div().text_xs().child(label))
            .into_any_element()
    }

    fn more_button(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        self.popup_trigger("ribbon-more", self.label("ui.more"), PopupKind::More, cx)
            .cursor_pointer()
            .flex()
            .items_center()
            .h(px(theme::CONTROL_HEIGHT))
            .px_2()
            .rounded_sm()
            .hover(|style| style.bg(rgb(theme::HOVER)))
            .child(self.label("ui.more"))
            .child(" ▾")
            .into_any_element()
    }
}

struct RibbonTooltip(String);
impl gpui::Render for RibbonTooltip {
    fn render(&mut self, _: &mut Window, _: &mut gpui::Context<Self>) -> impl IntoElement {
        div()
            .px_2()
            .py_1()
            .rounded_sm()
            .bg(rgb(theme::TEXT))
            .text_color(rgb(theme::WHITE))
            .text_sm()
            .child(self.0.clone())
    }
}

struct SharedStringStyle;
impl SharedStringStyle {
    fn id(id: &str) -> String {
        format!("gallery-{id}")
    }
}
