//! In-window prompt for Linux, where X11 and Wayland have no native one.
//!
//! GPUI's fallback box is a fixed 18 rem wide and lays the message and the
//! detail out as unconstrained flex rows, so a longer Chinese line is clipped
//! instead of wrapped. This prompt breaks both lines itself at the box width,
//! at the same opportunities the document layout uses, so closing punctuation
//! such as `。` never starts a line (GPUI's wrapper may break before any CJK
//! character). It answers Esc with the Cancel button and Enter with the Ok
//! button.

use std::ops::Range;

use gpui::{
    App, AppContext as _, Context, EventEmitter, FocusHandle, Focusable, InteractiveElement,
    IntoElement, KeyDownEvent, ParentElement, Pixels, PromptButton, PromptHandle, PromptLevel,
    PromptResponse, Render, RenderablePromptHandle, SharedString, StatefulInteractiveElement,
    Styled, Window, div, px, relative, rems, rgb, rgba,
};

const CARD_WIDTH: f32 = 440.0;

pub(crate) fn install(cx: &mut App) {
    cx.set_prompt_builder(build);
}

fn build(
    _level: PromptLevel,
    message: &str,
    detail: Option<&str>,
    actions: &[PromptButton],
    handle: PromptHandle,
    window: &mut Window,
    cx: &mut App,
) -> RenderablePromptHandle {
    let view = cx.new(|cx| WrappedPrompt {
        message: message.to_owned(),
        detail: detail.map(str::to_owned),
        actions: actions.to_vec(),
        focus: cx.focus_handle(),
    });
    handle.with_view(view, window, cx)
}

/// The button a bare Esc or Enter answers: Cancel for Esc, Ok for Enter.
/// Other keys, and a prompt without that kind of button, answer nothing.
pub(crate) fn key_answer(actions: &[PromptButton], key: &str) -> Option<usize> {
    match key {
        "escape" => actions.iter().position(PromptButton::is_cancel),
        "enter" => actions
            .iter()
            .position(|action| matches!(action, PromptButton::Ok(_))),
        _ => None,
    }
}

/// Lines of one paragraph `width` wide, as byte ranges that may end in
/// spaces. `x_at(byte)` is the pen position before `byte`. A line ends only
/// at [`joyword_layout::break_opportunities`], so a character that must not
/// start a line moves down together with the one before it. A first piece
/// wider than `width` stays whole on its own line.
pub(crate) fn wrap_ranges(
    text: &str,
    width: f32,
    x_at: impl Fn(usize) -> f32,
) -> Vec<Range<usize>> {
    let ends = joyword_layout::break_opportunities(text);
    let mut lines = Vec::new();
    let mut start = 0;
    while start < text.len() {
        let measured = |end: usize| {
            let visible = start.max(text[..end].trim_end().len());
            x_at(visible) - x_at(start)
        };
        let mut chosen = None;
        for &end in ends.iter().filter(|end| **end > start) {
            let fits = measured(end) <= width;
            if fits || chosen.is_none() {
                chosen = Some(end);
            }
            if !fits {
                break;
            }
        }
        let end = chosen.unwrap_or(text.len());
        lines.push(start..end);
        start = end;
    }
    lines
}

/// `text` broken into display lines for the current window font at `size`.
fn wrapped_lines(text: &str, size: Pixels, width: Pixels, window: &Window) -> Vec<SharedString> {
    let style = window.text_style();
    let mut lines = Vec::new();
    for paragraph in text.split('\n') {
        if paragraph.is_empty() {
            lines.push(SharedString::default());
            continue;
        }
        let shaped = window.text_system().shape_line(
            SharedString::from(paragraph.to_owned()),
            size,
            &[style.to_run(paragraph.len())],
            None,
        );
        for range in wrap_ranges(paragraph, f32::from(width), |at| {
            f32::from(shaped.x_for_index(at))
        }) {
            lines.push(SharedString::from(paragraph[range].trim_end().to_owned()));
        }
    }
    lines
}

fn line_divs(lines: Vec<SharedString>) -> impl Iterator<Item = gpui::Div> {
    lines.into_iter().map(|line| div().w_full().child(line))
}

struct WrappedPrompt {
    message: String,
    detail: Option<String>,
    actions: Vec<PromptButton>,
    focus: FocusHandle,
}

impl EventEmitter<PromptResponse> for WrappedPrompt {}

impl Focusable for WrappedPrompt {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for WrappedPrompt {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let rem = window.rem_size();
        let viewport = window.viewport_size().width * 0.9;
        let card = if viewport < px(CARD_WIDTH) {
            viewport
        } else {
            px(CARD_WIDTH)
        };
        // Padding on both sides, the border, and a pixel against rounding.
        let text_width = card - rem * 2.0 - px(3.0);
        let message_size = window.text_style().font_size.to_pixels(rem);
        let message = wrapped_lines(&self.message, message_size, text_width, window);
        let detail = self
            .detail
            .as_deref()
            .map(|detail| wrapped_lines(detail, rems(0.875).to_pixels(rem), text_width, window));
        let buttons = self.actions.iter().enumerate().map(|(index, action)| {
            let primary = !action.is_cancel();
            div()
                .id(index)
                .role(gpui::Role::Button)
                .aria_label(action.label().clone())
                .cursor_pointer()
                .px_3()
                .py_1()
                .rounded_sm()
                .text_sm()
                .bg(if primary {
                    rgb(0x163b65)
                } else {
                    rgb(0xe7eef5)
                })
                .text_color(if primary {
                    rgb(0xffffff)
                } else {
                    rgb(0x263247)
                })
                .on_click(cx.listener(move |_, _, _, cx| {
                    cx.emit(PromptResponse(index));
                    cx.stop_propagation();
                }))
                .child(action.label().clone())
        });
        let card = div()
            .id("joyword-prompt")
            .role(gpui::Role::Dialog)
            .aria_label(self.message.clone())
            .track_focus(&self.focus)
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                let mods = &event.keystroke.modifiers;
                if mods.control || mods.alt || mods.shift || mods.platform || mods.function {
                    return;
                }
                if let Some(index) = key_answer(&this.actions, &event.keystroke.key) {
                    cx.emit(PromptResponse(index));
                    cx.stop_propagation();
                }
            }))
            .w(px(CARD_WIDTH))
            .max_w(relative(0.9))
            .flex()
            .flex_col()
            .gap_2()
            .p_4()
            .rounded_md()
            .bg(rgb(0xffffff))
            .border_1()
            .border_color(rgb(0xc5ced6))
            .text_color(rgb(0x263247))
            .child(div().w_full().children(line_divs(message)))
            .children(detail.map(|detail| {
                div()
                    .w_full()
                    .text_sm()
                    .text_color(rgb(0x5c6b7a))
                    .children(line_divs(detail))
            }))
            .child(
                div()
                    .w_full()
                    .mt_2()
                    .flex()
                    .flex_wrap()
                    .justify_end()
                    .gap_2()
                    .children(buttons),
            );
        div()
            .id("joyword-prompt-overlay")
            .occlude()
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .bg(rgba(0x1b263480))
            .child(card)
    }
}
