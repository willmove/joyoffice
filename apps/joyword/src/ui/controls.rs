use super::theme;
use gpui::{
    App, ClickEvent, Context, IntoElement, KeyDownEvent, Render, Role, SharedString, Window, div,
    prelude::*, px, rgb,
};
use std::rc::Rc;

pub type Activate = Rc<dyn Fn(&mut Window, &mut App)>;

pub(crate) const CONTROL_CONTEXT: &str = "JoyWordControl";

pub(crate) fn named_control(
    id: impl Into<SharedString>,
    role: Role,
    name: impl Into<SharedString>,
) -> gpui::Stateful<gpui::Div> {
    let id = id.into();
    div()
        .id(id.clone())
        .accessibility_id(id)
        .role(role)
        .aria_label(name)
        .key_context(CONTROL_CONTEXT)
}

pub(crate) fn text_field(
    id: impl Into<SharedString>,
    name: impl Into<SharedString>,
    value: impl Into<SharedString>,
) -> gpui::Stateful<gpui::Div> {
    named_control(id, Role::TextInput, name).aria_value(value)
}

pub(crate) fn zoom_slider(value: u16, name: String) -> gpui::Stateful<gpui::Div> {
    named_control("zoom-slider", Role::Slider, name)
        .aria_numeric_value(f64::from(value))
        .aria_min_numeric_value(f64::from(crate::ZOOM_MIN))
        .aria_max_numeric_value(f64::from(crate::ZOOM_MAX))
        .aria_numeric_value_step(10.0)
        .aria_orientation(gpui::accesskit::Orientation::Horizontal)
        .aria_value(format!("{value}%"))
}

pub(crate) fn accessible_zoom(data: Option<&gpui::accesskit::ActionData>) -> Option<u16> {
    use gpui::accesskit::ActionData;
    let value = match data? {
        ActionData::NumericValue(value) => *value,
        ActionData::Value(value) => value.trim_end_matches('%').parse::<f64>().ok()?,
        _ => return None,
    };
    (value.is_finite()
        && (f64::from(crate::ZOOM_MIN)..=f64::from(crate::ZOOM_MAX)).contains(&value))
    .then(|| value.round() as u16)
}

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
            checkbox: false,
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
            checkbox: false,
        },
        activate,
    )
}

struct ButtonStyle {
    enabled: bool,
    pressed: bool,
    compact: bool,
    inverse: bool,
    checkbox: bool,
}

fn semantic_control(
    id: SharedString,
    name: SharedString,
    checkbox: bool,
    checked: bool,
) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id.clone())
        .accessibility_id(id)
        .role(if checkbox {
            Role::CheckBox
        } else {
            Role::Button
        })
        .aria_label(name)
        .key_context(CONTROL_CONTEXT)
        .when(checkbox, |element| element.aria_toggled(checked.into()))
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
        checkbox,
    } = style;
    let label = label.into();
    let tooltip = tooltip.into();
    let click = Rc::clone(&activate);
    let key = Rc::clone(&activate);
    // Tooltips contain the localized command name even when the visible
    // content is only an icon (e.g. quick-open). Expose that name to AT.
    semantic_control(id.into(), tooltip.clone(), checkbox, pressed)
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
    styled_button(
        id,
        format!("{} {label}", if checked { "☑" } else { "☐" }),
        label,
        ButtonStyle {
            enabled: true,
            pressed: checked,
            compact: true,
            inverse: false,
            checkbox: true,
        },
        activate,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::Element;

    #[test]
    fn document_bindings_do_not_match_a_focused_toolbar_or_field() -> Result<(), String> {
        use gpui::{KeyBindingContextPredicate, KeyContext};
        for predicate in [crate::DOCUMENT, crate::TYPING, crate::COMPOSING] {
            let predicate =
                KeyBindingContextPredicate::parse(predicate).map_err(|e| e.to_string())?;
            for control in [CONTROL_CONTEXT, crate::field_edit::FIELD_CONTEXT] {
                let contexts = [
                    KeyContext::parse("JoyWordInput composing").map_err(|e| e.to_string())?,
                    KeyContext::parse(control).map_err(|e| e.to_string())?,
                ];
                assert!(!predicate.eval(&contexts));
            }
        }
        let contexts = [KeyContext::parse("JoyWordInput").map_err(|e| e.to_string())?];
        assert!(
            KeyBindingContextPredicate::parse(crate::TYPING)
                .map_err(|e| e.to_string())?
                .eval(&contexts)
        );
        Ok(())
    }

    #[test]
    fn slider_exposes_percent_bounds_orientation_and_accepts_only_valid_values() {
        let element = zoom_slider(110, "缩放".into());
        assert_eq!(element.a11y_role(), Some(Role::Slider));
        let mut node = accesskit::Node::new(Role::Slider);
        element.write_a11y_info(&mut node);
        assert_eq!(node.role(), Role::Slider);
        assert_eq!(node.numeric_value(), Some(110.0));
        assert_eq!(node.min_numeric_value(), Some(f64::from(crate::ZOOM_MIN)));
        assert_eq!(node.max_numeric_value(), Some(f64::from(crate::ZOOM_MAX)));
        assert_eq!(node.orientation(), Some(accesskit::Orientation::Horizontal));
        assert_eq!(node.value(), Some("110%"));
        for value in [f64::NAN, f64::INFINITY, -1.0, 0.0, 10000.0] {
            assert_eq!(
                accessible_zoom(Some(&accesskit::ActionData::NumericValue(value))),
                None
            );
        }
        assert_eq!(
            accessible_zoom(Some(&accesskit::ActionData::Value("90%".into()))),
            Some(90)
        );
        assert_eq!(
            accessible_zoom(Some(&accesskit::ActionData::NumericValue(125.4))),
            Some(125)
        );
    }

    #[test]
    fn icon_buttons_publish_the_localized_command_name_and_stable_id() {
        let element = semantic_control("quick-open".into(), "打开文档".into(), false, false);
        assert_eq!(element.a11y_role(), Some(Role::Button));
        let mut node = accesskit::Node::new(Role::Button);
        element.write_a11y_info(&mut node);
        assert_eq!(node.label(), Some("打开文档"));
        assert_eq!(node.author_id(), Some("quick-open"));
        assert_eq!(node.toggled(), None);
    }

    #[test]
    fn checkbox_accessibility_tracks_both_checked_and_unchecked_states() {
        for checked in [false, true] {
            let element = semantic_control("show-marks".into(), "编辑标记".into(), true, checked);
            assert_eq!(element.a11y_role(), Some(Role::CheckBox));
            let mut node = accesskit::Node::new(Role::CheckBox);
            element.write_a11y_info(&mut node);
            assert_eq!(node.label(), Some("编辑标记"));
            assert_eq!(node.toggled(), Some(checked.into()));
        }
    }
}
