use super::theme;
use gpui::{
    App, ClickEvent, Context, IntoElement, KeyDownEvent, Render, SharedString, Window, div,
    prelude::*, px, rgb,
};
use std::rc::Rc;

pub type Activate = Rc<dyn Fn(&mut Window, &mut App)>;

struct TooltipText(String);

impl Render for TooltipText {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
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

pub fn button(
    id: impl Into<SharedString>,
    label: impl Into<SharedString>,
    tooltip: impl Into<SharedString>,
    enabled: bool,
    pressed: bool,
    compact: bool,
    activate: Activate,
) -> gpui::AnyElement {
    styled_button(
        id,
        label,
        tooltip,
        ButtonStyle {
            enabled,
            pressed,
            compact,
            inverse: false,
        },
        activate,
    )
}

pub fn title_button(
    id: impl Into<SharedString>,
    label: impl Into<SharedString>,
    tooltip: impl Into<SharedString>,
    enabled: bool,
    pressed: bool,
    compact: bool,
    activate: Activate,
) -> gpui::AnyElement {
    styled_button(
        id,
        label,
        tooltip,
        ButtonStyle {
            enabled,
            pressed,
            compact,
            inverse: true,
        },
        activate,
    )
}

struct ButtonStyle {
    enabled: bool,
    pressed: bool,
    compact: bool,
    inverse: bool,
}

fn styled_button(
    id: impl Into<SharedString>,
    label: impl Into<SharedString>,
    tooltip: impl Into<SharedString>,
    style: ButtonStyle,
    activate: Activate,
) -> gpui::AnyElement {
    let ButtonStyle {
        enabled,
        pressed,
        compact,
        inverse,
    } = style;
    let label = label.into();
    let tooltip = tooltip.into();
    let click = Rc::clone(&activate);
    let key = Rc::clone(&activate);
    div()
        .id(id.into())
        .focusable()
        .cursor_pointer()
        .flex()
        .items_center()
        .justify_center()
        .h(px(theme::CONTROL_HEIGHT))
        .px(if compact { px(5.0) } else { px(8.0) })
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
        .text_color(rgb(if !enabled {
            theme::TEXT_DISABLED
        } else if inverse {
            theme::WHITE
        } else {
            theme::TEXT
        }))
        .text_sm()
        .hover(|style| {
            style.bg(if !enabled {
                gpui::transparent_black().into()
            } else if inverse {
                rgb(theme::ACCENT_DARK)
            } else {
                rgb(theme::HOVER)
            })
        })
        .active(|style| style.bg(rgb(theme::PRESSED)))
        .focus(|style| style.border_color(rgb(theme::ACCENT_DARK)))
        .on_click(move |_: &ClickEvent, window, cx| {
            if enabled {
                click(window, cx);
            }
        })
        .on_key_down(move |event: &KeyDownEvent, window, cx| {
            if enabled && matches!(event.keystroke.key.as_str(), "enter" | "space") {
                key(window, cx);
                cx.stop_propagation();
            }
        })
        .tooltip(move |_, cx| cx.new(|_| TooltipText(tooltip.to_string())).into())
        .child(label)
        .into_any_element()
}

pub fn checkbox(
    id: impl Into<SharedString>,
    label: impl Into<SharedString>,
    checked: bool,
    activate: Activate,
) -> gpui::AnyElement {
    let label = label.into();
    button(
        id,
        format!("{} {label}", if checked { "☑" } else { "☐" }),
        label,
        true,
        checked,
        true,
        activate,
    )
}
