//! In-window prompt for Linux, where X11 and Wayland have no native one.
//!
//! GPUI's fallback box is a fixed 18 rem wide and lays the message and the
//! detail out as unconstrained flex rows, so a longer Chinese line is clipped
//! instead of wrapped. This prompt gives both lines the box width so they wrap,
//! and answers Esc with the Cancel button and Enter with the Ok button.

use gpui::{
    App, AppContext as _, Context, EventEmitter, FocusHandle, Focusable, InteractiveElement,
    IntoElement, KeyDownEvent, ParentElement, PromptButton, PromptHandle, PromptLevel,
    PromptResponse, Render, RenderablePromptHandle, StatefulInteractiveElement, Styled, Window,
    div, px, relative, rgb, rgba,
};

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
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let buttons = self.actions.iter().enumerate().map(|(index, action)| {
            let primary = !action.is_cancel();
            div()
                .id(index)
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
            .w(px(440.0))
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
            .child(div().w_full().child(self.message.clone()))
            .children(self.detail.clone().map(|detail| {
                div()
                    .w_full()
                    .text_sm()
                    .text_color(rgb(0x5c6b7a))
                    .child(detail)
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
