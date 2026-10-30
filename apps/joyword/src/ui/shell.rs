use crate::ui::controls::{self, Activate};
use crate::ui::theme;
use crate::{FindSlot, JoyWord, Popup, PopupKind, SidePanel, ZOOM_MAX, ZOOM_MIN};
use gpui::{
    Bounds, Context, Entity, MouseButton, Pixels, Point, Window, canvas, div, fill, point,
    prelude::*, px, rgb, size,
};
use joyword_foundation::Emu;
use joyword_shell::chrome;
use joyword_shell::command::*;
use joyword_shell::l10n::Catalog;
use std::rc::Rc;
use std::sync::Arc;

pub(crate) const START_MORE_CONTROLS: &[&str] = &[
    "start.paste",
    "start.cut",
    "start.copy",
    "start.format_copy",
    "start.bold",
    "start.italic",
    "start.underline",
    "start.font",
    "start.list_bullet",
    "start.list_level_1",
    "start.list_level_2",
    "start.list_remove",
    "start.align_left",
    "start.align",
    "start.align_right",
    "start.justify",
    "start.paragraph",
    "start.styles",
    "start.find_next",
    "start.replace",
];

fn activate(entity: &Entity<JoyWord>, command: CommandId) -> Activate {
    let entity = entity.clone();
    Rc::new(move |window, cx| {
        entity.update(cx, |this, cx| this.run(command, window, cx));
    })
}

fn close_and_run(entity: &Entity<JoyWord>, command: CommandId) -> Activate {
    let entity = entity.clone();
    Rc::new(move |window, cx| {
        entity.update(cx, |this, cx| {
            this.popup = None;
            this.run(command, window, cx);
        });
    })
}

impl JoyWord {
    pub(crate) fn open_popup(&mut self, kind: PopupKind, anchor: Point<Pixels>) {
        self.popup = Some(Popup { kind, anchor });
    }

    pub(crate) fn popup_trigger(
        &self,
        id: &'static str,
        label: String,
        kind: PopupKind,
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        // Capture this frame's label bounds so keyboard activation is anchored
        // to the control rather than to an unrelated mouse position.
        let anchor = Rc::new(std::cell::Cell::new(point(px(0.0), px(0.0))));
        let painted_anchor = anchor.clone();
        div()
            .on_children_prepainted(move |bounds, _, _| {
                if let Some(bounds) = bounds.first() {
                    painted_anchor.set(point(bounds.left(), bounds.bottom()));
                }
            })
            .id(id)
            .accessibility_id(id)
            .role(gpui::Role::Button)
            .aria_label(label)
            .key_context(controls::CONTROL_CONTEXT)
            .aria_expanded(self.popup.is_some_and(|popup| popup.kind == kind))
            .focusable()
            .on_click(cx.listener(move |this, event: &gpui::ClickEvent, _, cx| {
                this.open_popup(kind, event.position());
                cx.notify();
            }))
            .on_key_down(cx.listener(move |this, event: &gpui::KeyDownEvent, _, cx| {
                if matches!(event.keystroke.key.as_str(), "enter" | "space" | "down") {
                    this.open_popup(kind, anchor.get());
                    cx.stop_propagation();
                    cx.notify();
                }
            }))
    }

    pub(crate) fn show_find_panel(
        &mut self,
        replace: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.side_panel = Some(SidePanel::Find { replace });
        self.popup = None;
        window.focus(&self.find_query_focus, cx);
    }

    pub(crate) fn title_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let entity = cx.entity();
        let title = self
            .document_title()
            .unwrap_or_else(|| "JoyWord".to_owned());
        let edited = self
            .editor
            .as_ref()
            .is_some_and(|editor| editor.has_unsaved_changes());
        div()
            .flex()
            .items_center()
            .h(px(38.0))
            .px_2()
            .gap_1()
            .bg(rgb(theme::ACCENT))
            .text_color(rgb(theme::WHITE))
            .child(
                self.popup_trigger("file-menu", self.label("menu.file"), PopupKind::File, cx)
                    .cursor_pointer()
                    .px_3()
                    .py_1()
                    .rounded_sm()
                    .hover(|style| style.bg(rgb(theme::ACCENT_DARK)))
                    .child(self.label("menu.file")),
            )
            .child(controls::title_button(
                "quick-open",
                "↗",
                self.label("command.file.open"),
                true,
                false,
                true,
                activate(&entity, FILE_OPEN),
            ))
            .child(controls::title_button(
                "quick-save",
                "▣",
                self.label("command.file.save"),
                !matches!(
                    self.input_availability(FILE_SAVE),
                    Availability::Disabled(_)
                ),
                false,
                true,
                activate(&entity, FILE_SAVE),
            ))
            .child(controls::title_button(
                "quick-undo",
                "↶",
                self.label("command.edit.undo"),
                !matches!(
                    self.input_availability(EDIT_UNDO),
                    Availability::Disabled(_)
                ),
                false,
                true,
                activate(&entity, EDIT_UNDO),
            ))
            .child(controls::title_button(
                "quick-redo",
                "↷",
                self.label("command.edit.redo"),
                !matches!(
                    self.input_availability(EDIT_REDO),
                    Availability::Disabled(_)
                ),
                false,
                true,
                activate(&entity, EDIT_REDO),
            ))
            .child(
                div()
                    .flex_1()
                    .flex()
                    .justify_center()
                    .text_sm()
                    .child(format!(
                        "{}{title} - JoyWord",
                        if edited { "*" } else { "" }
                    )),
            )
            .child(
                div()
                    .w(px(170.0))
                    .flex()
                    .justify_end()
                    .text_xs()
                    .child(if edited {
                        self.label("input.dirty")
                    } else {
                        self.label("app.saved")
                    }),
            )
    }

    pub(crate) fn stage_notification(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .items_center()
            .justify_between()
            .px_3()
            .py_1()
            .bg(rgb(theme::NOTICE))
            .text_xs()
            .text_color(rgb(theme::TEXT))
            .child(self.label("stage"))
            .child(controls::button(
                "dismiss-stage",
                "×",
                self.label("ui.close"),
                true,
                false,
                true,
                Rc::new({
                    let entity = cx.entity();
                    move |_, cx| {
                        entity.update(cx, |this, cx| {
                            this.stage_notice = false;
                            cx.notify();
                        });
                    }
                }),
            ))
    }

    pub(crate) fn workspace_side_panel(&self, cx: &mut Context<Self>) -> Option<gpui::AnyElement> {
        match self.side_panel {
            Some(SidePanel::Styles) => Some(
                div()
                    .flex()
                    .flex_col()
                    .w(px(260.0))
                    .h_full()
                    .flex_shrink_0()
                    .child(self.panel_header("style.panel", cx))
                    .child(self.style_panel(cx).into_any_element())
                    .into_any_element(),
            ),
            Some(SidePanel::Find { replace }) => Some(
                div()
                    .flex()
                    .flex_col()
                    .w(px(300.0))
                    .h_full()
                    .flex_shrink_0()
                    .bg(rgb(theme::RIBBON))
                    .border_r_1()
                    .border_color(rgb(theme::BORDER))
                    .child(self.panel_header("find.query", cx))
                    .child(self.find_panel(replace, cx))
                    .into_any_element(),
            ),
            None => None,
        }
    }

    fn panel_header(&self, label: &'static str, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .items_center()
            .justify_between()
            .px_3()
            .py_2()
            .child(div().text_sm().child(self.label(label)))
            .child(controls::button(
                SharedStringPanel::close_id(label),
                "×",
                self.label("ui.close"),
                true,
                false,
                true,
                Rc::new({
                    let entity = cx.entity();
                    move |window, cx| {
                        entity.update(cx, |this, cx| {
                            this.side_panel = None;
                            window.focus(&this.focus_handle, cx);
                            cx.notify();
                        });
                    }
                }),
            ))
    }

    fn find_panel(&self, replace: bool, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .gap_2()
            .p_3()
            .text_sm()
            .child(self.find_field(FindSlot::Query, cx))
            .children(replace.then(|| self.find_field(FindSlot::Replacement, cx)))
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap_1()
                    .child(self.find_button(
                        "find-next",
                        &self.label("command.find.next"),
                        FIND_NEXT,
                        false,
                        cx,
                    ))
                    .child(self.find_button(
                        "find-previous",
                        &self.label("command.find.previous"),
                        FIND_PREVIOUS,
                        false,
                        cx,
                    ))
                    .children(replace.then(|| {
                        self.find_button(
                            "find-replace",
                            &self.label("command.find.replace"),
                            FIND_REPLACE,
                            true,
                            cx,
                        )
                        .into_any_element()
                    }))
                    .children(replace.then(|| {
                        self.find_button(
                            "find-replace-all",
                            &self.label("command.find.replace_all"),
                            FIND_REPLACE_ALL,
                            true,
                            cx,
                        )
                        .into_any_element()
                    })),
            )
            .child(self.find_option(crate::FindToggle::Case, cx))
            .child(self.find_option(crate::FindToggle::Word, cx))
            .child(self.find_option(crate::FindToggle::Width, cx))
            .child(self.find_option(crate::FindToggle::Regex, cx))
            .child(
                div()
                    .text_xs()
                    .text_color(rgb(theme::TEXT_MUTED))
                    .child(self.find_note.clone().unwrap_or_default()),
            )
    }

    fn find_option(&self, which: crate::FindToggle, cx: &mut Context<Self>) -> gpui::AnyElement {
        let key = match which {
            crate::FindToggle::Case => "find.case",
            crate::FindToggle::Word => "find.word",
            crate::FindToggle::Width => "find.width",
            crate::FindToggle::Regex => "find.regex",
        };
        let entity = cx.entity();
        controls::checkbox(
            format!("find-option-{key}"),
            self.label(key),
            self.find_toggle_on(which),
            Rc::new(move |_, cx| {
                entity.update(cx, |this, cx| {
                    this.toggle_find(which);
                    cx.notify();
                });
            }),
        )
    }

    pub(crate) fn status_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let entity = cx.entity();
        div()
            .flex()
            .items_center()
            .h(px(theme::STATUS_HEIGHT))
            .px_2()
            .gap_2()
            .bg(rgb(theme::WHITE))
            .border_t_1()
            .border_color(rgb(theme::BORDER))
            .text_xs()
            .child(
                div()
                    .flex_1()
                    .overflow_hidden()
                    .text_color(rgb(theme::TEXT_MUTED))
                    .child(self.input_status()),
            )
            .child(self.status_summary())
            .child(controls::button(
                "status-zoom-out",
                "−",
                self.label("command.view.zoom_out"),
                self.zoom_percent > ZOOM_MIN,
                false,
                true,
                activate(&entity, ZOOM_OUT),
            ))
            .child(self.zoom_slider(cx))
            .child(controls::button(
                "status-zoom-in",
                "+",
                self.label("command.view.zoom_in"),
                self.zoom_percent < ZOOM_MAX,
                false,
                true,
                activate(&entity, ZOOM_IN),
            ))
            .child(controls::button(
                "status-zoom-value",
                format!("{}%", self.zoom_percent),
                self.label("command.view.zoom_reset"),
                true,
                false,
                true,
                activate(&entity, ZOOM_RESET),
            ))
    }

    fn status_summary(&self) -> String {
        let words = self
            .editor
            .as_ref()
            .map(|editor| chrome::census(editor.view().index().text()).words())
            .unwrap_or_default();
        let page = self.ready_layout().and_then(|frame| {
            let index = self.editor.as_ref().and_then(|editor| {
                frame
                    .layout
                    .caret(editor.view().selection().head)
                    .ok()
                    .map(|caret| caret.page)
            });
            chrome::display_page(index, frame.layout.pages().len()).0
        });
        Catalog::shipped(self.locale).format(
            "status.compact",
            &[
                ("page", &page.map_or("–".into(), |page| page.to_string())),
                ("words", &words.to_string()),
            ],
        )
    }

    fn set_zoom_from_mouse(&mut self, position: Point<Pixels>, cx: &mut Context<Self>) {
        let Some(bounds) = self.zoom_bounds.lock().ok().and_then(|slot| *slot) else {
            return;
        };
        self.set_zoom_percent(zoom_from_x(bounds, position.x));
        cx.notify();
    }

    fn zoom_slider(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let slot = Arc::clone(&self.zoom_bounds);
        let zoom = self.zoom_percent;
        controls::zoom_slider(zoom, self.label("ribbon.group.zoom"))
            .on_a11y_action(gpui::AccessibleAction::SetValue, {
                let entity = cx.entity();
                move |data, _, cx| {
                    if let Some(value) = controls::accessible_zoom(data) {
                        entity.update(cx, |view, cx| {
                            view.set_zoom_percent(value);
                            cx.notify();
                        });
                    }
                }
            })
            .w(px(100.0))
            .h(px(18.0))
            .cursor_pointer()
            .focusable()
            .on_key_down(cx.listener(|this, event: &gpui::KeyDownEvent, _, cx| {
                let next = match event.keystroke.key.as_str() {
                    "left" | "down" => this.zoom_percent.saturating_sub(10).max(ZOOM_MIN),
                    "right" | "up" => this.zoom_percent.saturating_add(10).min(ZOOM_MAX),
                    "home" => ZOOM_MIN,
                    "end" => ZOOM_MAX,
                    _ => return,
                };
                this.set_zoom_percent(next);
                cx.stop_propagation();
                cx.notify();
            }))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, event: &gpui::MouseDownEvent, _, cx| {
                    this.zoom_dragging = true;
                    this.set_zoom_from_mouse(event.position, cx);
                }),
            )
            .on_mouse_move(cx.listener(|this, event: &gpui::MouseMoveEvent, _, cx| {
                if this.zoom_dragging && event.dragging() {
                    this.set_zoom_from_mouse(event.position, cx);
                }
            }))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    this.zoom_dragging = false;
                    cx.notify();
                }),
            )
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    this.zoom_dragging = false;
                    cx.notify();
                }),
            )
            .child(
                canvas(
                    |_, _, _| {},
                    move |bounds, _, window, _| {
                        if let Ok(mut current) = slot.lock() {
                            *current = Some(bounds);
                        }
                        let y = bounds.center().y;
                        window.paint_quad(fill(
                            Bounds::new(
                                point(bounds.origin.x, y - px(1.0)),
                                size(bounds.size.width, px(2.0)),
                            ),
                            rgb(theme::BORDER),
                        ));
                        let t = f32::from(zoom - ZOOM_MIN) / f32::from(ZOOM_MAX - ZOOM_MIN);
                        let x = bounds.origin.x + bounds.size.width * t;
                        window.paint_quad(fill(
                            Bounds::new(point(x - px(3.0), y - px(5.0)), size(px(6.0), px(10.0))),
                            rgb(theme::ACCENT),
                        ));
                    },
                )
                .size_full(),
            )
    }

    pub(crate) fn popup_overlay(&self, cx: &mut Context<Self>) -> Option<gpui::AnyElement> {
        let popup = self.popup?;
        let card = self.popup_card(popup.kind, cx);
        let menu_width = match popup.kind {
            PopupKind::TableGrid => 220.0,
            PopupKind::File | PopupKind::More => 244.0,
            _ => 236.0,
        };
        let left = self
            .viewport_bounds
            .map_or(popup.anchor.x, |viewport| {
                popup
                    .anchor
                    .x
                    .min(viewport.origin.x + viewport.size.width - px(menu_width))
            })
            .max(px(8.0));
        Some(
            div()
                .id("popup-overlay")
                .absolute()
                .inset_0()
                .size_full()
                .occlude()
                .on_key_down(cx.listener(|_, event: &gpui::KeyDownEvent, window, cx| {
                    match event.keystroke.key.as_str() {
                        "down" => window.focus_next(cx),
                        "up" => window.focus_prev(cx),
                        _ => return,
                    }
                    cx.stop_propagation();
                }))
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, _, _, cx| {
                        this.popup = None;
                        cx.notify();
                    }),
                )
                .child(
                    div()
                        .absolute()
                        .left(left)
                        .top(popup.anchor.y + px(4.0))
                        .on_mouse_down(MouseButton::Left, |_, _, cx| {
                            cx.stop_propagation();
                        })
                        .child(card),
                )
                .into_any_element(),
        )
    }

    fn popup_card(&self, kind: PopupKind, cx: &mut Context<Self>) -> gpui::AnyElement {
        let card = div()
            .id(SharedStringPopup::id(kind))
            .role(gpui::Role::Group)
            .aria_label(self.label(if matches!(kind, PopupKind::File) {
                "menu.file"
            } else {
                "ui.more"
            }))
            .flex()
            .flex_col()
            .gap_1()
            .p_2()
            .rounded_md()
            .bg(rgb(theme::WHITE))
            .border_1()
            .border_color(rgb(theme::BORDER))
            .shadow_lg()
            .text_sm();
        match kind {
            PopupKind::File => card
                .w(px(230.0))
                .children(
                    [
                        FILE_OPEN,
                        FILE_SAVE,
                        FILE_SAVE_COPY,
                        FILE_EXPORT_PDF,
                        FILE_EXPORT_PDFA,
                        FILE_EXPORT_EPUB,
                        FILE_MAIL_MERGE,
                        FILE_PRINT,
                        FILE_CLOSE,
                    ]
                    .into_iter()
                    .map(|command| self.popup_command(command, cx)),
                )
                .into_any_element(),
            PopupKind::FontFamily => card
                .w(px(220.0))
                .children(
                    [
                        "等线",
                        "宋体",
                        "微软雅黑",
                        "Noto Serif CJK SC",
                        "Arial",
                        "Times New Roman",
                    ]
                    .into_iter()
                    .map(|value| self.font_value_item("family", value, cx)),
                )
                .into_any_element(),
            PopupKind::FontSize => card
                .w(px(120.0))
                .children(
                    ["9", "10.5", "12", "14", "16", "18", "22", "24", "28", "36"]
                        .into_iter()
                        .map(|value| self.font_value_item("size", value, cx)),
                )
                .into_any_element(),
            PopupKind::FontColor => self.color_palette(card, cx),
            PopupKind::TableGrid => self.table_grid(card, cx),
            PopupKind::PageSize => card
                .w(px(180.0))
                .child(self.popup_ribbon("layout.a4", cx))
                .child(self.popup_ribbon("layout.letter", cx))
                .into_any_element(),
            PopupKind::Margins => card
                .w(px(180.0))
                .child(self.popup_ribbon("layout.margins", cx))
                .child(self.popup_ribbon("layout.margins_narrow", cx))
                .into_any_element(),
            PopupKind::Orientation => card
                .w(px(160.0))
                .child(self.popup_ribbon("layout.portrait", cx))
                .child(self.popup_ribbon("layout.landscape", cx))
                .into_any_element(),
            PopupKind::More => card
                .w(px(220.0))
                .max_h(px(480.0))
                .overflow_y_scroll()
                .children(
                    chrome::visible_controls(&self.registry, chrome::RIBBON)
                        .into_iter()
                        .filter(|control| {
                            control.tab == self.ribbon_tab
                                && (self.ribbon_tab != chrome::RibbonTab::Start
                                    || START_MORE_CONTROLS.contains(&control.id))
                        })
                        .map(|control| self.popup_ribbon(control.id, cx)),
                )
                .into_any_element(),
        }
    }

    fn popup_command(&self, command: CommandId, cx: &mut Context<Self>) -> gpui::AnyElement {
        let label = self
            .registry
            .get(command)
            .map(|spec| self.label(spec.label))
            .unwrap_or_else(|| command.0.to_owned());
        controls::button(
            format!("popup-{}", command.0),
            label.clone(),
            label,
            true,
            false,
            false,
            close_and_run(&cx.entity(), command),
        )
    }

    fn popup_ribbon(&self, id: &str, cx: &mut Context<Self>) -> gpui::AnyElement {
        let Some(control) = chrome::RIBBON
            .iter()
            .find(|control| control.id == id)
            .copied()
        else {
            return div().into_any_element();
        };
        let label = self.label(control.label);
        let entity = cx.entity();
        controls::button(
            format!("popup-{id}"),
            label.clone(),
            label,
            self.ribbon_enabled(control.command),
            false,
            false,
            Rc::new(move |window, cx| {
                entity.update(cx, |this, cx| {
                    this.popup = None;
                    this.run_ribbon(control, window, cx);
                });
            }),
        )
    }

    fn font_value_item(
        &self,
        name: &'static str,
        value: &'static str,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let entity = cx.entity();
        controls::button(
            format!("font-{name}-{value}"),
            value,
            value,
            true,
            false,
            false,
            Rc::new(move |window, cx| {
                entity.update(cx, |this, cx| {
                    this.popup = None;
                    this.apply_font_value(name, value, window, cx);
                });
            }),
        )
    }

    pub(crate) fn apply_font_value(
        &mut self,
        name: &'static str,
        value: &'static str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.run_with(
            FORMAT_FONT,
            &[
                ("commit", ArgValue::Text("apply".into())),
                (name, ArgValue::Text(value.into())),
            ],
            window,
            cx,
        );
    }

    fn color_palette(
        &self,
        card: gpui::Stateful<gpui::Div>,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let colors: &[(&str, u32)] = &[
            ("000000", 0x000000),
            ("404040", 0x404040),
            ("808080", 0x808080),
            ("FFFFFF", 0xffffff),
            ("C00000", 0xc00000),
            ("ED7D31", 0xed7d31),
            ("FFC000", 0xffc000),
            ("70AD47", 0x70ad47),
            ("4472C4", 0x4472c4),
            ("7030A0", 0x7030a0),
            ("5B9BD5", 0x5b9bd5),
            ("00B0F0", 0x00b0f0),
        ];
        card.w(px(172.0))
            .child(div().text_xs().child(self.label("ui.font_color")))
            .child(div().flex().flex_wrap().gap_1().children(colors.iter().map(
                |(value, color)| {
                    div()
                        .id(format!("color-{value}"))
                        .cursor_pointer()
                        .w(px(32.0))
                        .h(px(28.0))
                        .rounded_sm()
                        .border_1()
                        .border_color(rgb(theme::BORDER))
                        .bg(rgb(*color))
                        .hover(|style| style.border_color(rgb(theme::ACCENT)))
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.popup = None;
                            this.apply_font_value("color", value, window, cx);
                        }))
                        .into_any_element()
                },
            )))
            .child(self.font_value_item("color", "auto", cx))
            .into_any_element()
    }

    fn table_grid(
        &self,
        card: gpui::Stateful<gpui::Div>,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let (rows, columns) = self.table_preview;
        card.w(px(206.0))
            .child(div().text_xs().child(Catalog::shipped(self.locale).format(
                "ui.table_size",
                &[
                    ("rows", &rows.to_string()),
                    ("columns", &columns.to_string()),
                ],
            )))
            .children((1..=8).map(|row| {
                div()
                    .flex()
                    .gap_1()
                    .children((1..=8).map(|column| {
                        let selected = row <= rows && column <= columns;
                        let entity = cx.entity();
                        div()
                            .id(format!("table-{row}-{column}"))
                            .cursor_pointer()
                            .w(px(20.0))
                            .h(px(20.0))
                            .border_1()
                            .border_color(rgb(if selected {
                                theme::ACCENT
                            } else {
                                theme::BORDER
                            }))
                            .bg(rgb(if selected {
                                theme::SELECTED
                            } else {
                                theme::WHITE
                            }))
                            .on_hover({
                                let entity = entity.clone();
                                move |hovered, _, cx| {
                                    if *hovered {
                                        entity.update(cx, |this, cx| {
                                            this.table_preview = (row, column);
                                            cx.notify();
                                        });
                                    }
                                }
                            })
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.popup = None;
                                this.run_with(
                                    TABLE_INSERT,
                                    &[
                                        ("rows", ArgValue::Text(row.to_string())),
                                        ("columns", ArgValue::Text(column.to_string())),
                                    ],
                                    window,
                                    cx,
                                );
                            }))
                            .into_any_element()
                    }))
                    .into_any_element()
            }))
            .child(self.popup_command(TABLE_PROPERTIES, cx))
            .into_any_element()
    }
}

struct SharedStringPanel;
impl SharedStringPanel {
    fn close_id(label: &str) -> String {
        format!("close-panel-{label}")
    }
}

struct SharedStringPopup;
impl SharedStringPopup {
    fn id(kind: PopupKind) -> String {
        format!("popup-{kind:?}")
    }
}

pub(crate) fn zoom_from_x(bounds: Bounds<Pixels>, x: Pixels) -> u16 {
    let ratio = ((x - bounds.origin.x) / bounds.size.width.max(px(1.0))).clamp(0.0, 1.0);
    ZOOM_MIN + (ratio * f32::from(ZOOM_MAX - ZOOM_MIN)).round() as u16
}

pub(crate) fn ruler_page_geometry(
    viewport: Bounds<Pixels>,
    scroll: Point<Pixels>,
    page_width: Emu,
    left_margin: Emu,
    right_margin: Emu,
    zoom: u16,
) -> Option<(Pixels, Pixels)> {
    if page_width.raw() <= left_margin.raw().saturating_add(right_margin.raw()) {
        return None;
    }
    let scale = zoom as f64 / 100.0;
    let logical = |value: Emu| px((value.raw() as f64 / 914400.0 * 96.0 * scale) as f32);
    let centered = (f32::from(viewport.size.width)
        - 2.0 * crate::input_ui::DOC_PAD
        - f32::from(logical(page_width)))
    .max(0.0)
        / 2.0;
    let origin = viewport.origin.x
        + px(crate::input_ui::DOC_PAD + centered)
        + scroll.x
        + logical(left_margin);
    let width = logical(Emu::new(
        page_width.raw() - left_margin.raw() - right_margin.raw(),
    ));
    Some((origin, width))
}
