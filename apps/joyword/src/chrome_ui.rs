//! Ribbon commands, page-aligned ruler and formatting dialogs (T205).
//! Dialogs apply one undoable transaction; cancel applies none.
use crate::{DialogKind, FormatDialog, JoyWord, RulerHit, ZOOM_MAX, ZOOM_MIN};
use gpui::{
    Bounds, Context, MouseButton, PathBuilder, Pixels, Window, canvas, div, fill, point,
    prelude::*, px, rgb, size,
};
use joyword_foundation::Emu;
use joyword_model::TabAlign;
use joyword_shell::chrome::{
    self, RibbonArgs, RibbonControl, RibbonInput, RulerModel, RulerTab, icon_cells, ruler_scale,
};
use joyword_shell::command::{
    Availability, CommandId, EDIT_ALIGN_CENTER, EDIT_ALIGN_LEFT, EDIT_ALIGN_RIGHT, EDIT_BOLD,
    EDIT_ITALIC, EDIT_JUSTIFY, EDIT_UNDERLINE, REVIEW_FINAL, REVIEW_MARKUP, REVIEW_ORIGINAL,
    REVIEW_TRACK, VIEW_FIELD_CODES, VIEW_FORMATTING_MARKS, VIEW_HIDDEN_TEXT, ZOOM_IN, ZOOM_OUT,
};
use joyword_shell::input::RunToggle;

const INK: u32 = 0x163b65;

impl JoyWord {
    pub(crate) fn ribbon(&self, window: &Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.word_ribbon(window, cx)
    }

    /// Word shows character formats, the paragraph alignment and view
    /// switches as pressed buttons at the caret or selection.
    pub(crate) fn ribbon_pressed(&self, id: CommandId) -> bool {
        let toggles = self.view_toggles;
        let alignment =
            |wanted: joyword_model::Justification| self.caret_alignment() == Some(wanted);
        let shows = |toggle: RunToggle| {
            self.editor
                .as_ref()
                .is_some_and(|editor| editor.shows(toggle))
        };
        match id {
            EDIT_BOLD => shows(RunToggle::Bold),
            EDIT_ITALIC => shows(RunToggle::Italic),
            EDIT_UNDERLINE => shows(RunToggle::Underline),
            EDIT_ALIGN_LEFT => alignment(joyword_model::Justification::Start),
            EDIT_ALIGN_CENTER => alignment(joyword_model::Justification::Center),
            EDIT_ALIGN_RIGHT => alignment(joyword_model::Justification::End),
            EDIT_JUSTIFY => alignment(joyword_model::Justification::Both),
            VIEW_FORMATTING_MARKS => toggles.formatting_marks,
            VIEW_HIDDEN_TEXT => toggles.show_hidden,
            VIEW_FIELD_CODES => toggles.field_codes,
            REVIEW_MARKUP => toggles.revision == joyword_model::RevisionView::FinalWithMarkup,
            REVIEW_FINAL => toggles.revision == joyword_model::RevisionView::Final,
            REVIEW_ORIGINAL => toggles.revision == joyword_model::RevisionView::Original,
            REVIEW_TRACK => self
                .editor
                .as_ref()
                .is_some_and(|editor| editor.tracking_enabled()),
            _ => false,
        }
    }

    /// Resolved alignment of the caret's paragraph; no value is left/start.
    fn caret_alignment(&self) -> Option<joyword_model::Justification> {
        let editor = self.editor.as_ref()?;
        let head = editor.view().selection().head;
        let document = editor.view().snapshot();
        let paragraph = document.find_paragraph(head.story, head.paragraph)?;
        Some(
            document
                .styles()
                .resolve_paragraph(paragraph.style.as_ref(), &paragraph.props)
                .props
                .justification
                .unwrap_or(joyword_model::Justification::Start),
        )
    }

    pub(crate) fn ribbon_enabled(&self, id: CommandId) -> bool {
        if matches!(self.input_availability(id), Availability::Disabled(_)) {
            return false;
        }
        if id == ZOOM_IN && self.zoom_percent >= ZOOM_MAX {
            return false;
        }
        if id == ZOOM_OUT && self.zoom_percent <= ZOOM_MIN {
            return false;
        }
        true
    }

    pub(crate) fn run_ribbon(
        &mut self,
        control: RibbonControl,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if matches!(control.args, RibbonArgs::PromptPath) {
            self.prompt_insert_picture(cx);
            return;
        }
        let call = control.bind(RibbonInput {
            query: self.find_query.text(),
            replacement: self.find_replacement.text(),
            case_sensitive: self.find_case,
            whole_word: self.find_word,
            format_repeat: self.format_repeat,
        });
        if call.args.is_empty() {
            self.run(call.command, window, cx);
        } else {
            self.run_with(call.command, &call.args, window, cx);
        }
    }

    pub(crate) fn ruler(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let model = self.current_ruler();
        let unresolved = model.unresolved;
        let bounds_slot = std::sync::Arc::clone(&self.ruler_bounds);
        let painted = model.clone();
        let geometry = self.ruler_screen_geometry();
        div()
            .flex()
            .items_center()
            .h(px(crate::ui::theme::RULER_HEIGHT))
            .bg(rgb(crate::ui::theme::WHITE))
            .border_b_1()
            .border_color(rgb(crate::ui::theme::BORDER))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, event: &gpui::MouseDownEvent, _, cx| {
                    this.begin_ruler_drag(event.position, cx);
                }),
            )
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, event: &gpui::MouseUpEvent, _, cx| {
                    this.finish_ruler_drag(event.position, false, cx);
                }),
            )
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(|this, event: &gpui::MouseUpEvent, _, cx| {
                    this.finish_ruler_drag(event.position, true, cx);
                }),
            )
            .child(
                div()
                    .relative()
                    .flex_1()
                    .h_full()
                    .child(
                        canvas(
                            |_, _, _| {},
                            move |bounds, _, window, _| {
                                let content = geometry.map_or(bounds, |(origin, width)| {
                                    Bounds::new(
                                        point(origin, bounds.origin.y),
                                        size(width, bounds.size.height),
                                    )
                                });
                                if let Ok(mut slot) = bounds_slot.lock() {
                                    *slot = Some(content);
                                }
                                let scale = painted
                                    .content_width
                                    .unwrap_or_else(|| ruler_scale(&painted));
                                paint_ruler(content, scale, &painted, window);
                            },
                        )
                        .absolute()
                        .top_0()
                        .left_0()
                        .size_full(),
                    )
                    .children(ruler_labels(&model, geometry)),
            )
            .children(unresolved.then(|| {
                div()
                    .px_2()
                    .text_xs()
                    .text_color(rgb(0x8a5a00))
                    .child(self.label("ruler.unresolved"))
            }))
    }

    fn ruler_screen_geometry(&self) -> Option<(Pixels, Pixels)> {
        let frame = self.ready_layout()?;
        let count = frame.layout.pages().len();
        let index = self
            .editor
            .as_ref()
            .and_then(|editor| {
                frame
                    .layout
                    .caret(editor.view().selection().head)
                    .ok()
                    .map(|caret| caret.page)
            })
            .unwrap_or(0)
            .min(count.checked_sub(1)?);
        let page = frame.layout.page_geometry().get(index)?;
        crate::ui::shell::ruler_page_geometry(
            self.viewport_bounds?,
            self.scroll.offset(),
            page.width,
            page.left,
            page.right,
            self.zoom_percent,
        )
    }

    fn current_ruler(&self) -> RulerModel {
        let Some(editor) = self.editor.as_ref() else {
            return RulerModel::default();
        };
        let view = editor.view();
        let head = view.selection().head;
        let document = view.snapshot();
        let Some(paragraph) = document.find_paragraph(head.story, head.paragraph) else {
            return RulerModel::default();
        };
        let resolved = document
            .styles()
            .resolve_paragraph(paragraph.style.as_ref(), paragraph.props.as_ref());
        let tabs = resolved.props.tabs.clone().unwrap_or_default();
        let (interval, unresolved_interval) =
            chrome::default_tab_interval(document.settings().default_tab_stop);
        let mut model = chrome::ruler_model(
            &resolved.props.indent,
            &tabs,
            interval,
            self.content_width_emu(),
        );
        if unresolved_interval {
            model.unresolved = true;
        }
        model
    }

    fn content_width_emu(&self) -> Option<i64> {
        let frame = self.ready_layout()?;
        let count = frame.layout.pages().len();
        if count == 0 {
            return None;
        }
        let index = self
            .editor
            .as_ref()
            .and_then(|editor| {
                frame
                    .layout
                    .caret(editor.view().selection().head)
                    .ok()
                    .map(|caret| caret.page)
            })
            .unwrap_or(0)
            .min(count - 1);
        let geometry = frame.layout.page_geometry().get(index)?;
        let width = geometry
            .width
            .raw()
            .saturating_sub(geometry.left.raw())
            .saturating_sub(geometry.right.raw());
        (width > 0).then_some(width)
    }

    pub(crate) fn open_font_dialog(&mut self, cx: &mut Context<Self>) {
        let (family, size, bold, italic, underline, color) = self.font_prefill();
        self.format_dialog_focused = false;
        self.format_dialog = Some(FormatDialog {
            kind: DialogKind::Font,
            fields: vec![
                dialog_field("family", "dialog.font.family", family, cx),
                dialog_field("size", "dialog.font.size", size, cx),
                dialog_field("color", "dialog.font.color", color, cx),
            ],
            bold,
            italic,
            underline,
        });
        cx.notify();
    }

    pub(crate) fn open_paragraph_dialog(&mut self, cx: &mut Context<Self>) {
        let (align, start, end, first, hanging, before, after, line) = self.paragraph_prefill();
        self.format_dialog_focused = false;
        self.format_dialog = Some(FormatDialog {
            kind: DialogKind::Paragraph,
            fields: vec![
                dialog_field("align", "dialog.paragraph.align", align, cx),
                dialog_field("start", "dialog.paragraph.start", start, cx),
                dialog_field("end", "dialog.paragraph.end", end, cx),
                dialog_field("first", "dialog.paragraph.first", first, cx),
                dialog_field("hanging", "dialog.paragraph.hanging", hanging, cx),
                dialog_field("before", "dialog.paragraph.before", before, cx),
                dialog_field("after", "dialog.paragraph.after", after, cx),
                dialog_field("line", "dialog.paragraph.line", line, cx),
            ],
            bold: false,
            italic: false,
            underline: false,
        });
        cx.notify();
    }

    pub(crate) fn open_table_dialog(&mut self, cx: &mut Context<Self>) {
        let (width, align, top, left, bottom, right, border, size, color) = self.table_prefill();
        self.format_dialog_focused = false;
        self.format_dialog = Some(FormatDialog {
            kind: DialogKind::Table,
            fields: vec![
                dialog_field("width", "dialog.table.width", width, cx),
                dialog_field("align", "dialog.table.align", align, cx),
                dialog_field("top", "dialog.table.top", top, cx),
                dialog_field("left", "dialog.table.left", left, cx),
                dialog_field("bottom", "dialog.table.bottom", bottom, cx),
                dialog_field("right", "dialog.table.right", right, cx),
                dialog_field("border", "dialog.table.border", border, cx),
                dialog_field("border_size", "dialog.table.border_size", size, cx),
                dialog_field("border_color", "dialog.table.border_color", color, cx),
            ],
            bold: false,
            italic: false,
            underline: false,
        });
        cx.notify();
    }

    pub(crate) fn format_panel(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<gpui::AnyElement> {
        if !self.format_dialog_focused {
            let focus = self
                .format_dialog
                .as_ref()
                .and_then(|dialog| dialog.fields.first())
                .map(|field| field.focus.clone());
            self.format_dialog_focused = true;
            if let Some(focus) = focus {
                window.focus(&focus, cx);
            }
        }
        let dialog = self.format_dialog.as_ref()?;
        let kind = dialog.kind;
        let title = match kind {
            DialogKind::Font => self.label("command.format.font"),
            DialogKind::Paragraph => self.label("command.format.paragraph"),
            DialogKind::Table => self.label("command.table.properties"),
        };
        let mut panel = div()
            .id("format-dialog")
            .tab_group()
            .flex()
            .flex_col()
            .gap_2()
            .w(px(560.0))
            .max_h(px(680.0))
            .overflow_y_scroll()
            .p_4()
            .rounded_md()
            .bg(rgb(crate::ui::theme::WHITE))
            .border_1()
            .border_color(rgb(crate::ui::theme::BORDER))
            .shadow_lg()
            .child(div().text_lg().child(title));
        for index in 0..dialog.fields.len() {
            panel = panel.child(self.dialog_field(index, cx));
        }
        panel = panel.children(self.dialog_choices(kind, cx));
        if kind == DialogKind::Font {
            panel = panel.child(
                div()
                    .flex()
                    .gap_2()
                    .child(self.flag_button("dialog-bold", "dialog.font.bold", 0, cx))
                    .child(self.flag_button("dialog-italic", "dialog.font.italic", 1, cx))
                    .child(self.flag_button("dialog-underline", "dialog.font.underline", 2, cx)),
            );
        }
        let panel = panel.child(
            div()
                .flex()
                .justify_end()
                .gap_2()
                .child(self.dialog_action("dialog-cancel", "dialog.cancel", false, cx))
                .child(self.dialog_action("dialog-ok", "dialog.ok", true, cx)),
        );
        Some(
            div()
                .id("format-dialog-overlay")
                .absolute()
                .inset_0()
                .size_full()
                .occlude()
                .flex()
                .items_center()
                .justify_center()
                .bg(gpui::rgba(0x00000040))
                .child(panel)
                .into_any_element(),
        )
    }

    fn dialog_choices(&self, kind: DialogKind, cx: &mut Context<Self>) -> Option<gpui::AnyElement> {
        let rows: Vec<(usize, &[(&str, &str)])> = match kind {
            DialogKind::Font => vec![
                (0, &[("等线", "等线"), ("宋体", "宋体"), ("Arial", "Arial")]),
                (
                    1,
                    &[("10.5", "10.5"), ("12", "12"), ("14", "14"), ("16", "16")],
                ),
                (
                    2,
                    &[
                        ("auto", "ui.color_auto"),
                        ("000000", "ui.color_black"),
                        ("4472C4", "ui.color_blue"),
                    ],
                ),
            ],
            DialogKind::Paragraph => vec![(
                0,
                &[
                    ("start", "command.edit.align_left"),
                    ("center", "command.edit.align_center"),
                    ("end", "command.edit.align_right"),
                    ("both", "command.edit.justify"),
                ],
            )],
            DialogKind::Table => vec![(
                1,
                &[
                    ("left", "command.edit.align_left"),
                    ("center", "command.edit.align_center"),
                    ("right", "command.edit.align_right"),
                ],
            )],
        };
        Some(
            div()
                .flex()
                .flex_col()
                .gap_1()
                .children(rows.into_iter().map(|(index, choices)| {
                    div()
                        .flex()
                        .flex_wrap()
                        .gap_1()
                        .children(
                            choices
                                .iter()
                                .map(|(value, label)| self.dialog_choice(index, value, label, cx)),
                        )
                        .into_any_element()
                }))
                .into_any_element(),
        )
    }

    fn dialog_choice(
        &self,
        index: usize,
        value: &'static str,
        label: &'static str,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let text = if label.contains('.') {
            self.label(label)
        } else {
            label.to_owned()
        };
        div()
            .id(format!("dialog-choice-{index}-{value}"))
            .focusable()
            .cursor_pointer()
            .px_2()
            .py_1()
            .rounded_sm()
            .bg(rgb(crate::ui::theme::HOVER))
            .hover(|style| style.bg(rgb(crate::ui::theme::SELECTED)))
            .on_click(cx.listener(move |this, _, _, cx| {
                if let Some(dialog) = &mut this.format_dialog
                    && let Some(field) = dialog.fields.get_mut(index)
                {
                    field.value.set_text(value);
                }
                cx.notify();
            }))
            .on_key_down(cx.listener(move |this, event: &gpui::KeyDownEvent, _, cx| {
                if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                    if let Some(dialog) = &mut this.format_dialog
                        && let Some(field) = dialog.fields.get_mut(index)
                    {
                        field.value.set_text(value);
                    }
                    cx.stop_propagation();
                    cx.notify();
                }
            }))
            .child(text)
            .into_any_element()
    }

    fn dialog_field(&self, index: usize, cx: &mut Context<Self>) -> gpui::AnyElement {
        let Some(field) = self
            .format_dialog
            .as_ref()
            .and_then(|dialog| dialog.fields.get(index))
        else {
            return div().into_any_element();
        };
        let focus = field.focus.clone();
        let name = field.name;
        let label = self.label(field.label);
        let shown = crate::field_edit::field_view(&field.value, &label);
        div()
            .flex()
            .items_center()
            .gap_2()
            .child(div().w(px(120.0)).text_xs().child(label.clone()))
            .child(
                div()
                    .id(name)
                    .track_focus(&focus)
                    .key_context(crate::field_edit::FIELD_CONTEXT)
                    .focusable()
                    .flex_1()
                    .px_2()
                    .py_1()
                    .border_1()
                    .border_color(rgb(0xc5ced6))
                    .bg(rgb(0xffffff))
                    .text_color(rgb(0x263247))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        if let Some(dialog) = &this.format_dialog
                            && let Some(field) = dialog.fields.get(index)
                        {
                            window.focus(&field.focus, cx);
                        }
                    }))
                    .on_key_down(cx.listener(
                        move |this, event: &gpui::KeyDownEvent, window, cx| {
                            this.type_dialog_field(index, event, window, cx);
                        },
                    ))
                    .child(shown),
            )
            .into_any_element()
    }

    fn flag_button(
        &self,
        id: &'static str,
        label: &'static str,
        flag: usize,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let on = self
            .format_dialog
            .as_ref()
            .is_some_and(|dialog| match flag {
                0 => dialog.bold,
                1 => dialog.italic,
                _ => dialog.underline,
            });
        let label = format!("{} {}", if on { "☑" } else { "☐" }, self.label(label));
        div()
            .id(id)
            .focusable()
            .cursor_pointer()
            .px_2()
            .py_1()
            .rounded_sm()
            .bg(if on { rgb(0xc5d7ea) } else { rgb(0xe7eef5) })
            .on_click(cx.listener(move |this, _, _, cx| {
                if let Some(dialog) = &mut this.format_dialog {
                    match flag {
                        0 => dialog.bold = !dialog.bold,
                        1 => dialog.italic = !dialog.italic,
                        _ => dialog.underline = !dialog.underline,
                    }
                }
                cx.notify();
            }))
            .on_key_down(cx.listener(move |this, event: &gpui::KeyDownEvent, _, cx| {
                if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                    if let Some(dialog) = &mut this.format_dialog {
                        match flag {
                            0 => dialog.bold = !dialog.bold,
                            1 => dialog.italic = !dialog.italic,
                            _ => dialog.underline = !dialog.underline,
                        }
                    }
                    cx.stop_propagation();
                    cx.notify();
                }
            }))
            .child(label)
    }

    fn dialog_action(
        &self,
        id: &'static str,
        label: &'static str,
        apply: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let label = self.label(label);
        div()
            .id(id)
            .focusable()
            .cursor_pointer()
            .px_3()
            .py_1()
            .rounded_sm()
            .bg(rgb(if apply {
                crate::ui::theme::ACCENT
            } else {
                crate::ui::theme::HOVER
            }))
            .text_color(rgb(if apply {
                crate::ui::theme::WHITE
            } else {
                crate::ui::theme::TEXT
            }))
            .focus(|style| {
                style
                    .border_1()
                    .border_color(rgb(crate::ui::theme::ACCENT_DARK))
            })
            .on_click(cx.listener(move |this, _, window, cx| {
                if apply {
                    this.apply_format_dialog(window, cx);
                } else {
                    this.close_format_dialog(window, cx);
                }
            }))
            .on_key_down(
                cx.listener(move |this, event: &gpui::KeyDownEvent, window, cx| {
                    if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                        if apply {
                            this.apply_format_dialog(window, cx);
                        } else {
                            this.close_format_dialog(window, cx);
                        }
                        cx.stop_propagation();
                    }
                }),
            )
            .child(label)
    }

    pub(crate) fn close_format_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.format_dialog = None;
        self.format_dialog_focused = false;
        window.focus(&self.focus_handle, cx);
        cx.notify();
    }

    /// Esc that no field consumed closes the format panel or the path sheet,
    /// like Cancel. While an IME composition is open Esc is bound to
    /// `CancelComposition` instead and never arrives here.
    pub(crate) fn escape_panel(
        &mut self,
        event: &gpui::KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let mods = &event.keystroke.modifiers;
        if event.keystroke.key != "escape"
            || mods.control
            || mods.alt
            || mods.shift
            || mods.platform
            || mods.function
        {
            return;
        }
        if self.popup.is_some() {
            self.popup = None;
            window.focus(&self.focus_handle, cx);
            cx.notify();
        } else if self.format_dialog.is_some() {
            self.close_format_dialog(window, cx);
        } else if self.path_sheet.is_some() {
            self.cancel_path_sheet(window, cx);
        } else if self.side_panel.take().is_some() {
            window.focus(&self.focus_handle, cx);
            cx.notify();
        } else {
            return;
        }
        cx.stop_propagation();
    }

    fn type_dialog_field(
        &mut self,
        index: usize,
        event: &gpui::KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let effect = {
            let Some(dialog) = &mut self.format_dialog else {
                return;
            };
            let Some(field) = dialog.fields.get_mut(index) else {
                return;
            };
            let effect = field.value.apply_key(&event.keystroke);
            crate::field_edit::consume(effect, &mut field.value, cx)
        };
        let Some(effect) = effect else {
            return;
        };
        cx.stop_propagation();
        match effect {
            crate::field_edit::FieldEffect::Cancel => self.close_format_dialog(window, cx),
            crate::field_edit::FieldEffect::Submit => self.apply_format_dialog(window, cx),
            _ => cx.notify(),
        }
    }

    fn apply_format_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(dialog) = &self.format_dialog else {
            return;
        };
        let command = match dialog.kind {
            DialogKind::Font => joyword_shell::command::FORMAT_FONT,
            DialogKind::Paragraph => joyword_shell::command::FORMAT_PARAGRAPH,
            DialogKind::Table => joyword_shell::command::TABLE_PROPERTIES,
        };
        let mut args = vec![(
            "commit",
            joyword_shell::command::ArgValue::Text("apply".into()),
        )];
        for field in &dialog.fields {
            let value = dialog_command_value(dialog.kind, field.name, field.value.text());
            args.push((field.name, joyword_shell::command::ArgValue::Text(value)));
        }
        if dialog.kind == DialogKind::Font {
            args.push((
                "bold",
                joyword_shell::command::ArgValue::Text(
                    if dialog.bold { "true" } else { "false" }.into(),
                ),
            ));
            args.push((
                "italic",
                joyword_shell::command::ArgValue::Text(
                    if dialog.italic { "true" } else { "false" }.into(),
                ),
            ));
            args.push((
                "underline",
                joyword_shell::command::ArgValue::Text(
                    if dialog.underline { "single" } else { "none" }.into(),
                ),
            ));
        }
        self.run_with(command, &args, window, cx);
        if self.input_error.is_none() {
            self.format_dialog = None;
        }
        cx.notify();
    }

    fn begin_ruler_drag(&mut self, position: gpui::Point<Pixels>, cx: &mut Context<Self>) {
        let Some(bounds) = self.ruler_bounds.lock().ok().and_then(|slot| *slot) else {
            return;
        };
        let model = self.current_ruler();
        let Some(hit) = hit_ruler(&model, bounds, position) else {
            return;
        };
        self.ruler_drag = Some((hit, position));
        cx.notify();
    }

    fn finish_ruler_drag(
        &mut self,
        position: gpui::Point<Pixels>,
        outside: bool,
        cx: &mut Context<Self>,
    ) {
        let Some((hit, origin)) = self.ruler_drag.take() else {
            return;
        };
        let moved = (f32::from(position.x - origin.x)).abs() >= 3.0
            || (f32::from(position.y - origin.y)).abs() >= 3.0;
        if !moved && !outside {
            return;
        }
        let Some(bounds) = self.ruler_bounds.lock().ok().and_then(|slot| *slot) else {
            return;
        };
        let model = self.current_ruler();
        let scale = model.content_width.unwrap_or_else(|| ruler_scale(&model));
        let position_emu = x_to_emu(bounds, scale, position.x);
        let drop_tab = outside && (f32::from(position.y - bounds.bottom())).abs() > 24.0;
        let drag = match hit {
            RulerHit::First => joyword_shell::dialogs::RulerDrag::Indent {
                marker: joyword_shell::dialogs::IndentMarker::FirstLine,
                position_emu,
                content_width_emu: model.content_width,
            },
            RulerHit::Body => joyword_shell::dialogs::RulerDrag::Indent {
                marker: joyword_shell::dialogs::IndentMarker::Body,
                position_emu,
                content_width_emu: model.content_width,
            },
            RulerHit::Right => joyword_shell::dialogs::RulerDrag::Indent {
                marker: joyword_shell::dialogs::IndentMarker::Right,
                position_emu,
                content_width_emu: model.content_width,
            },
            RulerHit::Tab { from_emu } => joyword_shell::dialogs::RulerDrag::Tab {
                from_emu,
                to_emu: if drop_tab { None } else { Some(position_emu) },
            },
        };
        self.apply_input(cx, |editor| editor.apply_ruler(drag));
    }

    pub(crate) fn font_prefill(&self) -> (String, String, bool, bool, bool, String) {
        let empty = (
            String::new(),
            String::new(),
            false,
            false,
            false,
            String::new(),
        );
        let Some(editor) = &self.editor else {
            return empty;
        };
        let view = editor.view();
        let head = view.selection().head;
        let document = view.snapshot();
        let Some(paragraph) = document.find_paragraph(head.story, head.paragraph) else {
            return empty;
        };
        let (style, props) = paragraph.format_at(head.offset);
        let resolved =
            document
                .styles()
                .resolve_run(paragraph.style.as_ref(), style.as_ref(), props.as_ref());
        let run = &resolved.props;
        let family = named_font(&run.fonts.east_asia).or_else(|| named_font(&run.fonts.ascii));
        (
            family.unwrap_or_default(),
            size_text(run.size),
            run.bold == Some(true),
            run.italic == Some(true),
            run.underline
                .is_some_and(|underline| underline != joyword_model::Underline::None),
            color_text(run.color),
        )
    }

    fn paragraph_prefill(
        &self,
    ) -> (
        String,
        String,
        String,
        String,
        String,
        String,
        String,
        String,
    ) {
        let empty = (
            String::new(),
            String::new(),
            String::new(),
            String::new(),
            String::new(),
            String::new(),
            String::new(),
            String::new(),
        );
        let Some(editor) = &self.editor else {
            return empty;
        };
        let head = editor.view().selection().head;
        let document = editor.view().snapshot();
        let Some(paragraph) = document.find_paragraph(head.story, head.paragraph) else {
            return empty;
        };
        let props = paragraph.props.as_ref();
        (
            align_text(props.justification),
            twip_text(props.indent.start),
            twip_text(props.indent.end),
            twip_text(props.indent.first_line),
            twip_text(props.indent.hanging),
            twip_text(props.spacing.before),
            twip_text(props.spacing.after),
            line_text(props.spacing.line),
        )
    }

    fn table_prefill(&self) -> TablePrefill {
        let empty = (
            String::new(),
            String::new(),
            String::new(),
            String::new(),
            String::new(),
            String::new(),
            String::new(),
            String::new(),
            String::new(),
        );
        let Some(editor) = &self.editor else {
            return empty;
        };
        let head = editor.view().selection().head;
        let document = editor.view().snapshot();
        let Ok(address) = joyword_engine::cell_address(document, &head) else {
            return empty;
        };
        let Some(story) = document.story(head.story) else {
            return empty;
        };
        let Some(joyword_model::Block::Table(table)) = story
            .blocks
            .iter()
            .find(|block| block.id() == address.table)
        else {
            return empty;
        };
        let (top, left, bottom, right) = match table.cell_margin {
            Some(margin) => (
                twips_display(margin.top),
                twips_display(margin.left),
                twips_display(margin.bottom),
                twips_display(margin.right),
            ),
            None => (String::new(), String::new(), String::new(), String::new()),
        };
        let (border, size, color) = shared_border(table)
            .map(|edge| {
                (
                    border_kind_text(edge.kind).to_owned(),
                    decimal_display(i64::from(edge.size_eighths), 8),
                    border_color_text(edge.color),
                )
            })
            .unwrap_or_default();
        (
            width_text(table.width),
            align_table_text(table.alignment),
            top,
            left,
            bottom,
            right,
            border,
            size,
            color,
        )
    }
}

type TablePrefill = (
    String,
    String,
    String,
    String,
    String,
    String,
    String,
    String,
    String,
);

fn width_text(width: Option<joyword_model::TableWidth>) -> String {
    match width {
        None => String::new(),
        Some(joyword_model::TableWidth::Auto) => "auto".to_owned(),
        Some(joyword_model::TableWidth::Dxa(value)) => format!("dxa:{value}"),
        Some(joyword_model::TableWidth::Pct(value)) => format!("pct:{value}"),
    }
}

fn align_table_text(alignment: Option<joyword_model::TableAlign>) -> String {
    match alignment {
        None => String::new(),
        Some(joyword_model::TableAlign::Left) => "left".to_owned(),
        Some(joyword_model::TableAlign::Center) => "center".to_owned(),
        Some(joyword_model::TableAlign::Right) => "right".to_owned(),
    }
}

fn shared_border(table: &joyword_model::Table) -> Option<joyword_model::BorderEdge> {
    let mut found = None;
    for row in &table.rows {
        for cell in &row.cells {
            for edge in [
                cell.borders.top,
                cell.borders.left,
                cell.borders.bottom,
                cell.borders.right,
            ] {
                let edge = edge?;
                match found {
                    None => found = Some(edge),
                    Some(current) if current == edge => {}
                    Some(_) => return None,
                }
            }
        }
    }
    found
}

fn border_kind_text(kind: joyword_model::BorderKind) -> &'static str {
    match kind {
        joyword_model::BorderKind::Nil => "nil",
        joyword_model::BorderKind::Single => "single",
        joyword_model::BorderKind::Double => "double",
        joyword_model::BorderKind::Dotted => "dotted",
        joyword_model::BorderKind::Dashed => "dashed",
        joyword_model::BorderKind::Thick => "thick",
    }
}

fn border_color_text(color: joyword_model::Color) -> String {
    match color {
        joyword_model::Color::Auto => "auto".to_owned(),
        joyword_model::Color::Rgb(red, green, blue) => {
            format!("{red:02X}{green:02X}{blue:02X}")
        }
        joyword_model::Color::Theme { .. } => String::new(),
    }
}

fn dialog_field(
    name: &'static str,
    label: &'static str,
    text: String,
    cx: &mut Context<JoyWord>,
) -> crate::DialogField {
    crate::DialogField {
        name,
        label,
        value: crate::field_edit::FieldBuffer::from_text(text),
        focus: cx.focus_handle(),
    }
}

fn named_font(font: &Option<joyword_model::FontRef>) -> Option<String> {
    match font {
        Some(joyword_model::FontRef::Named(name)) => Some(name.to_string()),
        _ => None,
    }
}

fn size_text(measure: Option<joyword_model::Measure>) -> String {
    let Some(joyword_model::Measure::HalfPoints(value)) = measure else {
        return String::new();
    };
    if let Some(name) = joyword_shell::dialogs::chinese_name_for_half_points(value) {
        return name.to_owned();
    }
    if value % 2 == 0 {
        format!("{}", value / 2)
    } else {
        format!("{}.5", value / 2)
    }
}

fn color_text(color: Option<joyword_model::Color>) -> String {
    match color {
        Some(joyword_model::Color::Rgb(red, green, blue)) => {
            format!("{red:02X}{green:02X}{blue:02X}")
        }
        Some(joyword_model::Color::Auto) => "auto".to_owned(),
        _ => String::new(),
    }
}

fn align_text(value: Option<joyword_model::Justification>) -> String {
    match value {
        Some(joyword_model::Justification::Start) => "start",
        Some(joyword_model::Justification::Center) => "center",
        Some(joyword_model::Justification::End) => "end",
        Some(joyword_model::Justification::Both) => "both",
        Some(joyword_model::Justification::Distribute) => "distribute",
        None => "",
    }
    .to_owned()
}

fn decimal_display(value: i64, divisor: i64) -> String {
    let absolute = value.unsigned_abs();
    let whole = absolute / divisor.unsigned_abs();
    let thousandths = absolute % divisor.unsigned_abs() * 1000 / divisor.unsigned_abs();
    let sign = if value < 0 { "-" } else { "" };
    if thousandths == 0 {
        format!("{sign}{whole}")
    } else {
        let fraction = format!("{thousandths:03}");
        format!("{sign}{whole}.{}", fraction.trim_end_matches('0'))
    }
}

fn twips_display(value: i64) -> String {
    decimal_display(value, 20)
}

fn parse_scaled(text: &str, multiplier: i64) -> String {
    if text.trim().is_empty() {
        return String::new();
    }
    text.trim()
        .parse::<f64>()
        .ok()
        .filter(|value| value.is_finite())
        .map(|value| (value * multiplier as f64).round().to_string())
        .unwrap_or_else(|| text.to_owned())
}

pub(crate) fn dialog_command_value(kind: DialogKind, name: &str, text: &str) -> String {
    let paragraph_twips = matches!(
        name,
        "start" | "end" | "first" | "hanging" | "before" | "after"
    );
    let table_twips = matches!(name, "top" | "left" | "bottom" | "right");
    if (kind == DialogKind::Paragraph && paragraph_twips)
        || (kind == DialogKind::Table && table_twips)
    {
        return parse_scaled(text, 20);
    }
    if kind == DialogKind::Table && name == "border_size" {
        return parse_scaled(text, 8);
    }
    if kind == DialogKind::Paragraph && name == "line" {
        for prefix in ["exact:", "atLeast:"] {
            if let Some(value) = text.strip_prefix(prefix) {
                return format!("{prefix}{}", parse_scaled(value, 20));
            }
        }
    }
    text.to_owned()
}

fn twip_text(measure: Option<joyword_model::Measure>) -> String {
    match measure {
        Some(joyword_model::Measure::Twips(value)) => twips_display(value),
        _ => String::new(),
    }
}

fn line_text(line: Option<joyword_model::LineSpacing>) -> String {
    match line {
        Some(joyword_model::LineSpacing::Auto(value)) => format!("auto:{value}"),
        Some(joyword_model::LineSpacing::Exact(joyword_model::Measure::Twips(value))) => {
            format!("exact:{}", twips_display(value))
        }
        Some(joyword_model::LineSpacing::AtLeast(joyword_model::Measure::Twips(value))) => {
            format!("atLeast:{}", twips_display(value))
        }
        _ => String::new(),
    }
}

fn hit_ruler(
    model: &RulerModel,
    bounds: Bounds<Pixels>,
    position: gpui::Point<Pixels>,
) -> Option<RulerHit> {
    let scale = model.content_width.unwrap_or_else(|| ruler_scale(model));
    let distance = |emu: i64| (f32::from(position.x - emu_x(bounds, scale, emu))).abs();
    let top = position.y < bounds.center().y;
    if top {
        if let Some(mark) = model.first_line
            && distance(mark) <= 8.0
        {
            return Some(RulerHit::First);
        }
    } else {
        let mut best: Option<(f32, RulerHit)> = None;
        if let Some(mark) = model.left
            && distance(mark) <= 8.0
        {
            best = Some((distance(mark), RulerHit::Body));
        }
        if let Some(mark) = model.right
            && distance(mark) <= 8.0
        {
            let next = distance(mark);
            if best.as_ref().is_none_or(|(previous, _)| next < *previous) {
                best = Some((next, RulerHit::Right));
            }
        }
        if let Some((_, hit)) = best {
            return Some(hit);
        }
    }
    let mut tab_hit: Option<(f32, i64)> = None;
    for tab in &model.tabs {
        let next = distance(tab.position);
        if next <= 6.0
            && tab_hit
                .as_ref()
                .is_none_or(|(previous, _)| next < *previous)
        {
            tab_hit = Some((next, tab.position));
        }
    }
    tab_hit.map(|(_, from_emu)| RulerHit::Tab { from_emu })
}

fn x_to_emu(bounds: Bounds<Pixels>, scale: i64, x: Pixels) -> i64 {
    let width = f32::from(bounds.size.width).max(1.0);
    let t = (f32::from(x - bounds.origin.x) / width).clamp(0.0, 1.0);
    (t * scale as f32).round() as i64
}

pub(crate) fn drawn_icon(id: &str, ink: u32) -> gpui::AnyElement {
    let cells = icon_cells(id);
    canvas(
        |_, _, _| {},
        move |bounds, _, window, _| {
            for (column, row) in &cells {
                let origin = point(
                    bounds.origin.x + bounds.size.width * (f32::from(*column) / 16.0),
                    bounds.origin.y + bounds.size.height * (f32::from(*row) / 16.0),
                );
                let step_x = bounds.size.width / px(16.0);
                let step_y = bounds.size.height / px(16.0);
                window.paint_quad(fill(
                    Bounds {
                        origin,
                        size: size(px(step_x.max(1.0)), px(step_y.max(1.0))),
                    },
                    rgb(ink),
                ));
            }
        },
    )
    .w(px(16.0))
    .h(px(16.0))
    .into_any_element()
}

fn ruler_labels(model: &RulerModel, geometry: Option<(Pixels, Pixels)>) -> Vec<gpui::AnyElement> {
    let Some((origin, width)) = geometry else {
        return Vec::new();
    };
    let scale = model.content_width.unwrap_or_else(|| ruler_scale(model));
    let mut labels = Vec::new();
    let mut value = 0;
    let mut inch = 0;
    while value <= scale {
        let x = origin + width * (value as f32 / scale.max(1) as f32);
        if x + px(12.0) <= origin + width {
            labels.push(
                div()
                    .absolute()
                    .left(x + px(2.0))
                    .top_0()
                    .text_xs()
                    .text_color(rgb(crate::ui::theme::TEXT_MUTED))
                    .child(inch.to_string())
                    .into_any_element(),
            );
        }
        inch += 1;
        value = value.saturating_add(Emu::PER_INCH);
    }
    labels
}

fn paint_ruler(bounds: Bounds<Pixels>, scale: i64, model: &RulerModel, window: &mut Window) {
    window.paint_quad(fill(
        Bounds {
            origin: point(
                bounds.origin.x,
                bounds.origin.y + bounds.size.height - px(1.0),
            ),
            size: size(bounds.size.width, px(1.0)),
        },
        rgb(0xd5dbe3),
    ));
    if model.content_width.is_some() {
        let mut inch = Emu::PER_INCH;
        while inch < scale {
            paint_tick(bounds, scale, inch, px(8.0), 0xc5ced6, window);
            let half = inch.saturating_sub(Emu::PER_INCH / 2);
            paint_tick(bounds, scale, half, px(5.0), 0xd2d0ce, window);
            let next = inch.saturating_add(Emu::PER_INCH);
            if next <= inch {
                break;
            }
            inch = next;
        }
    }
    for tab in &model.tabs {
        paint_tab(bounds, scale, tab, window);
    }
    if let Some(position) = model.left {
        paint_block(bounds, scale, position, false, INK, window);
    }
    if let Some(position) = model.first_line {
        paint_block(bounds, scale, position, true, 0x1f5b9b, window);
    }
    if let Some(position) = model.right {
        paint_block(bounds, scale, position, false, 0xc47b2b, window);
    }
}

fn paint_tab(bounds: Bounds<Pixels>, scale: i64, tab: &RulerTab, window: &mut Window) {
    let color = if tab.custom { 0x1f5b9b } else { 0xc5ced6 };
    let height = match tab.align {
        TabAlign::Bar => bounds.size.height,
        TabAlign::Center | TabAlign::Decimal | TabAlign::Num => px(10.0),
        TabAlign::Right => px(6.0),
        TabAlign::Left | TabAlign::Clear => px(6.0),
    };
    let top = match tab.align {
        TabAlign::Bar => bounds.origin.y,
        TabAlign::Right => bounds.origin.y + px(2.0),
        _ => bounds.origin.y + bounds.size.height - height - px(1.0),
    };
    let x = emu_x(bounds, scale, tab.position);
    window.paint_quad(fill(
        Bounds {
            origin: point(x, top),
            size: size(px(1.5), height),
        },
        rgb(color),
    ));
}

fn paint_tick(
    bounds: Bounds<Pixels>,
    scale: i64,
    emu: i64,
    height: Pixels,
    color: u32,
    window: &mut Window,
) {
    let x = emu_x(bounds, scale, emu);
    window.paint_quad(fill(
        Bounds {
            origin: point(x, bounds.origin.y),
            size: size(px(1.0), height),
        },
        rgb(color),
    ));
}

fn paint_block(
    bounds: Bounds<Pixels>,
    scale: i64,
    emu: i64,
    top: bool,
    color: u32,
    window: &mut Window,
) {
    let x = emu_x(bounds, scale, emu);
    let y = if top {
        bounds.origin.y + px(2.0)
    } else {
        bounds.origin.y + bounds.size.height - px(8.0)
    };
    let points = if top {
        [
            point(x - px(4.0), y),
            point(x + px(4.0), y),
            point(x, y + px(7.0)),
        ]
    } else {
        [
            point(x - px(4.0), y + px(7.0)),
            point(x + px(4.0), y + px(7.0)),
            point(x, y),
        ]
    };
    let mut builder = PathBuilder::fill();
    builder.add_polygon(&points, true);
    if let Ok(path) = builder.build() {
        window.paint_path(path, rgb(color));
    }
}

fn emu_x(bounds: Bounds<Pixels>, scale: i64, emu: i64) -> Pixels {
    let t = (emu as f32 / scale.max(1) as f32).clamp(0.0, 1.0);
    bounds.origin.x + bounds.size.width * t
}
