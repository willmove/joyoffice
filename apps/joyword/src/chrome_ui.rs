//! Ribbon, horizontal ruler and status counts (T205). Buttons call the
//! command registry. The ruler drags direct indents and tab stops. Font and
//! paragraph dialogs apply one undoable transaction; cancel applies none.
use crate::{DialogKind, FormatDialog, JoyWord, RulerHit, ZOOM_MAX, ZOOM_MIN};
use gpui::{
    Bounds, Context, MouseButton, Pixels, Window, canvas, div, fill, point, prelude::*, px, rgb,
    size,
};
use joyword_foundation::Emu;
use joyword_model::TabAlign;
use joyword_shell::chrome::{
    self, RibbonControl, RibbonInput, RibbonTab, RulerModel, RulerTab, icon_cells, ruler_scale,
    status_line,
};
use joyword_shell::command::{Availability, CommandId, ZOOM_IN, ZOOM_OUT};
use joyword_shell::l10n::Catalog;

const INK: u32 = 0x163b65;
const INK_MUTED: u32 = 0x9aa7b2;

impl JoyWord {
    pub(crate) fn ribbon(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let shown = chrome::visible_controls(&self.registry, chrome::RIBBON);
        let on_tab = chrome::controls_on(self.ribbon_tab, shown.iter().copied());
        let grouped = chrome::groups(&on_tab);
        div()
            .flex()
            .flex_col()
            .bg(rgb(0xf7f9fb))
            .border_b_1()
            .border_color(rgb(0xd5dbe3))
            .child(self.ribbon_tabs(cx))
            .child(
                div()
                    .id("ribbon-row")
                    .flex()
                    .items_end()
                    .gap_2()
                    .px_2()
                    .py_1()
                    .overflow_x_scroll()
                    .children(
                        grouped
                            .into_iter()
                            .map(|(group, controls)| self.ribbon_group(group, &controls, cx)),
                    ),
            )
    }

    fn ribbon_tabs(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .gap_1()
            .px_3()
            .pt_2()
            .children(RibbonTab::ALL.into_iter().map(|tab| {
                let selected = self.ribbon_tab == tab;
                div()
                    .id(tab.label_key())
                    .px_3()
                    .py_1()
                    .cursor_pointer()
                    .rounded_t_sm()
                    .bg(if selected {
                        rgb(0xffffff)
                    } else {
                        rgb(0xf7f9fb)
                    })
                    .text_color(if selected { rgb(INK) } else { rgb(0x5c6b7a) })
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.ribbon_tab = tab;
                        cx.notify();
                    }))
                    .child(self.label(tab.label_key()))
            }))
    }

    fn ribbon_group(
        &self,
        group: &'static str,
        controls: &[&RibbonControl],
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        div()
            .id(group)
            .flex()
            .flex_col()
            .items_center()
            .gap_1()
            .px_2()
            .border_r_1()
            .border_color(rgb(0xd5dbe3))
            .child(
                div().flex().gap_1().children(
                    controls
                        .iter()
                        .map(|control| self.ribbon_button(control, cx)),
                ),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(rgb(0x5c6b7a))
                    .child(self.label(group)),
            )
            .into_any_element()
    }

    fn ribbon_button(&self, control: &RibbonControl, cx: &mut Context<Self>) -> gpui::AnyElement {
        let control = *control;
        let enabled = self.ribbon_enabled(control.command);
        let ink = if enabled { INK } else { INK_MUTED };
        div()
            .id(control.id)
            .flex()
            .flex_col()
            .items_center()
            .gap_1()
            .px_2()
            .py_1()
            .rounded_sm()
            .cursor_pointer()
            .bg(if enabled {
                rgb(0xe7eef5)
            } else {
                rgb(0xf4f6f8)
            })
            .text_color(rgb(ink))
            .on_click(cx.listener(move |this, _, window, cx| {
                this.run_ribbon(control, window, cx);
            }))
            .child(drawn_icon(control.icon, ink))
            .child(div().text_xs().child(self.label(control.label)))
            .into_any_element()
    }

    fn ribbon_enabled(&self, id: CommandId) -> bool {
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

    fn run_ribbon(&mut self, control: RibbonControl, window: &mut Window, cx: &mut Context<Self>) {
        let call = control.bind(RibbonInput {
            query: &self.find_query,
            replacement: &self.find_replacement,
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
        div()
            .flex()
            .items_center()
            .h(px(28.0))
            .bg(rgb(0xffffff))
            .border_b_1()
            .border_color(rgb(0xd5dbe3))
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
                div().relative().flex_1().h_full().child(
                    canvas(
                        |_, _, _| {},
                        move |bounds, _, window, _| {
                            if let Ok(mut slot) = bounds_slot.lock() {
                                *slot = Some(bounds);
                            }
                            paint_ruler(bounds, &painted, window);
                        },
                    )
                    .absolute()
                    .top_0()
                    .left_0()
                    .size_full(),
                ),
            )
            .children(unresolved.then(|| {
                div()
                    .px_2()
                    .text_xs()
                    .text_color(rgb(0x8a5a00))
                    .child(self.label("ruler.unresolved"))
            }))
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

    pub(crate) fn status_counts(&self) -> String {
        let census = self
            .editor
            .as_ref()
            .map(|editor| chrome::census(editor.view().index().text()))
            .unwrap_or_default();
        let (page, pages) = self
            .ready_layout()
            .map(|frame| {
                let index = self.editor.as_ref().and_then(|editor| {
                    frame
                        .layout
                        .caret(editor.view().selection().head)
                        .ok()
                        .map(|caret| caret.page)
                });
                chrome::display_page(index, frame.layout.pages().len())
            })
            .unwrap_or((None, None));
        status_line(
            Catalog::shipped(self.locale),
            &chrome::StatusSnapshot {
                page,
                pages,
                census,
            },
        )
    }

    pub(crate) fn open_font_dialog(&mut self, cx: &mut Context<Self>) {
        let (family, size, bold, italic, underline, color) = self.font_prefill();
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

    pub(crate) fn format_panel(&self, cx: &mut Context<Self>) -> Option<gpui::AnyElement> {
        let dialog = self.format_dialog.as_ref()?;
        let kind = dialog.kind;
        let title = match kind {
            DialogKind::Font => self.label("command.format.font"),
            DialogKind::Paragraph => self.label("command.format.paragraph"),
        };
        let mut panel = div()
            .id("format-dialog")
            .flex()
            .flex_col()
            .gap_2()
            .px_4()
            .py_3()
            .bg(rgb(0xffffff))
            .border_t_1()
            .border_color(rgb(0xd5dbe3))
            .child(div().text_sm().child(title));
        for index in 0..dialog.fields.len() {
            panel = panel.child(self.dialog_field(index, cx));
        }
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
        Some(
            panel
                .child(
                    div()
                        .flex()
                        .gap_2()
                        .child(self.dialog_action("dialog-ok", "dialog.ok", true, cx))
                        .child(self.dialog_action("dialog-cancel", "dialog.cancel", false, cx)),
                )
                .into_any_element(),
        )
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
        let shown = if field.text.is_empty() {
            label.clone()
        } else {
            field.text.clone()
        };
        let faded = field.text.is_empty();
        div()
            .flex()
            .items_center()
            .gap_2()
            .child(div().w(px(120.0)).text_xs().child(label))
            .child(
                div()
                    .id(name)
                    .track_focus(&focus)
                    .focusable()
                    .flex_1()
                    .px_2()
                    .py_1()
                    .border_1()
                    .border_color(rgb(0xc5ced6))
                    .bg(rgb(0xffffff))
                    .text_color(if faded { rgb(0x8896a3) } else { rgb(0x263247) })
                    .on_click(cx.listener(move |this, _, window, cx| {
                        if let Some(dialog) = &this.format_dialog
                            && let Some(field) = dialog.fields.get(index)
                        {
                            window.focus(&field.focus, cx);
                        }
                    }))
                    .on_key_down(cx.listener(move |this, event: &gpui::KeyDownEvent, _, cx| {
                        this.type_dialog_field(index, event, cx);
                    }))
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
        let label = self.label(label);
        div()
            .id(id)
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
            .cursor_pointer()
            .px_3()
            .py_1()
            .rounded_sm()
            .bg(rgb(0x163b65))
            .text_color(rgb(0xffffff))
            .on_click(cx.listener(move |this, _, window, cx| {
                if apply {
                    this.apply_format_dialog(window, cx);
                } else {
                    this.format_dialog = None;
                    cx.notify();
                }
            }))
            .child(label)
    }

    fn type_dialog_field(
        &mut self,
        index: usize,
        event: &gpui::KeyDownEvent,
        cx: &mut Context<Self>,
    ) {
        let mods = &event.keystroke.modifiers;
        if mods.control || mods.alt || mods.platform || mods.function {
            return;
        }
        if event.keystroke.key == "escape" {
            self.format_dialog = None;
            cx.stop_propagation();
            cx.notify();
            return;
        }
        let Some(dialog) = &mut self.format_dialog else {
            return;
        };
        let Some(field) = dialog.fields.get_mut(index) else {
            return;
        };
        if event.keystroke.key == "backspace" {
            field.text.pop();
        } else if let Some(ch) = event.keystroke.key_char.as_deref() {
            if ch.chars().any(char::is_control) {
                return;
            }
            field.text.push_str(ch);
        } else {
            return;
        }
        cx.stop_propagation();
        cx.notify();
    }

    fn apply_format_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(dialog) = &self.format_dialog else {
            return;
        };
        let command = match dialog.kind {
            DialogKind::Font => joyword_shell::command::FORMAT_FONT,
            DialogKind::Paragraph => joyword_shell::command::FORMAT_PARAGRAPH,
        };
        let mut args = vec![(
            "commit",
            joyword_shell::command::ArgValue::Text("apply".into()),
        )];
        for field in &dialog.fields {
            args.push((
                field.name,
                joyword_shell::command::ArgValue::Text(field.text.clone()),
            ));
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
        let scale = ruler_scale(&model);
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

    fn font_prefill(&self) -> (String, String, bool, bool, bool, String) {
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
        text,
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

fn twip_text(measure: Option<joyword_model::Measure>) -> String {
    match measure {
        Some(joyword_model::Measure::Twips(value)) => value.to_string(),
        _ => String::new(),
    }
}

fn line_text(line: Option<joyword_model::LineSpacing>) -> String {
    match line {
        Some(joyword_model::LineSpacing::Auto(value)) => format!("auto:{value}"),
        Some(joyword_model::LineSpacing::Exact(joyword_model::Measure::Twips(value))) => {
            format!("exact:{value}")
        }
        Some(joyword_model::LineSpacing::AtLeast(joyword_model::Measure::Twips(value))) => {
            format!("atLeast:{value}")
        }
        _ => String::new(),
    }
}

fn hit_ruler(
    model: &RulerModel,
    bounds: Bounds<Pixels>,
    position: gpui::Point<Pixels>,
) -> Option<RulerHit> {
    let scale = ruler_scale(model);
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

fn drawn_icon(id: &str, ink: u32) -> gpui::AnyElement {
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

fn paint_ruler(bounds: Bounds<Pixels>, model: &RulerModel, window: &mut Window) {
    let scale = ruler_scale(model);
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
    window.paint_quad(fill(
        Bounds {
            origin: point(x - px(3.0), y),
            size: size(px(6.0), px(6.0)),
        },
        rgb(color),
    ));
}

fn emu_x(bounds: Bounds<Pixels>, scale: i64, emu: i64) -> Pixels {
    let t = (emu as f32 / scale.max(1) as f32).clamp(0.0, 1.0);
    bounds.origin.x + bounds.size.width * t
}
