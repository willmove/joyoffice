//! Shared multi-language state and scalable locale picker controls for JoyOffice.
use crate::{IconName, Palette};
use gpui_kit::component::{
    Icon, Selectable, Sizable,
    button::{Button, ButtonVariants},
    popover::Popover,
};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;
use joyoffice_l10n::{UiLocale, shared_text};

/// Process-wide active UI locale, mirrored across open windows of an application.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AppliedLocale(pub UiLocale);
impl Global for AppliedLocale {}

/// Update the active UI locale and refresh every window of the current app.
pub fn apply_locale(locale: UiLocale, cx: &mut App) {
    cx.set_global(AppliedLocale(locale));
    cx.refresh_windows();
}

/// Read the active UI locale, defaulting to Simplified Chinese when unset.
pub fn current_locale(cx: &App) -> UiLocale {
    cx.try_global::<AppliedLocale>()
        .map_or(UiLocale::default(), |applied| applied.0)
}

/// Localized theme preset label backed by `joyoffice-l10n`.
pub fn theme_label(preset: joyoffice_theme::ThemePreset, locale: UiLocale) -> &'static str {
    let label = joyoffice_l10n::theme_label(preset.id(), locale);
    if label.is_empty() {
        preset.label(locale.is_zh_cn())
    } else {
        label
    }
}

/// Compact header/titlebar trigger button for [`language_menu`].
pub fn language_header_trigger(
    id: impl Into<ElementId>,
    current: UiLocale,
    palette: Palette,
) -> Button {
    let tooltip = shared_text(current, "language.switch_tooltip");
    let aria = format!(
        "{} ({})",
        shared_text(current, "language.selector_aria"),
        current.native_name()
    );
    Button::new(id)
        .ghost()
        .xsmall()
        .icon(IconName::Globe)
        .label(current.short_label())
        .dropdown_caret(true)
        .h(px(26.))
        .px_2()
        .text_color(rgb(palette.text))
        .accessibility_label(aria)
        .tooltip(tooltip)
}

/// Compact status-bar trigger button for [`language_menu`].
pub fn language_status_trigger(
    id: impl Into<ElementId>,
    current: UiLocale,
    palette: Palette,
) -> Button {
    let tooltip = shared_text(current, "language.switch_tooltip");
    let aria = format!(
        "{} ({})",
        shared_text(current, "language.selector_aria"),
        current.native_name()
    );
    Button::new(id)
        .ghost()
        .xsmall()
        .icon(IconName::Globe)
        .label(current.native_name())
        .h(px(20.))
        .px_1()
        .text_size(px(11.))
        .text_color(rgb(palette.text))
        .accessibility_label(aria)
        .tooltip(tooltip)
}

/// Popover language selector listing every shipped [`UiLocale`] by native autonym.
///
/// The list scrolls vertically once many languages are registered, so adding
/// dozens of mainstream locales requires no layout changes in calling apps.
pub fn language_menu<F>(
    id: impl Into<ElementId>,
    current: UiLocale,
    palette: Palette,
    trigger: Button,
    on_select: F,
) -> Popover
where
    F: Fn(UiLocale, &mut Window, &mut App) + Clone + 'static,
{
    Popover::new(id).trigger(trigger).content(move |_, _, cx| {
        let popup = cx.entity().downgrade();
        div()
            .id("joyoffice-language-menu-list")
            .w(px(220.))
            .max_h(px(320.))
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .gap(px(2.))
            .children(UiLocale::ALL.into_iter().map(|locale| {
                let selected = locale == current;
                let popup = popup.clone();
                let on_select = on_select.clone();
                let aria = format!("{} ({})", locale.native_name(), locale.tag());
                Button::new(SharedString::from(format!("lang-menu-{}", locale.tag())))
                    .ghost()
                    .small()
                    .w_full()
                    .h(px(30.))
                    .px_2()
                    .selected(selected)
                    .toggled(selected)
                    .accessibility_label(aria)
                    .child(
                        div()
                            .w_full()
                            .flex()
                            .items_center()
                            .justify_between()
                            .gap_2()
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .child(
                                        div()
                                            .w(px(14.))
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .when(selected, |el| {
                                                el.child(
                                                    Icon::new(IconName::Check)
                                                        .size(px(13.))
                                                        .text_color(rgb(palette.accent)),
                                                )
                                            }),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(12.5))
                                            .when(selected, |el| {
                                                el.font_weight(FontWeight::SEMIBOLD)
                                                    .text_color(rgb(palette.accent))
                                            })
                                            .child(locale.native_name()),
                                    ),
                            )
                            .child(
                                div()
                                    .text_size(px(11.))
                                    .text_color(rgb(palette.muted))
                                    .child(locale.tag()),
                            ),
                    )
                    .on_click(move |_, w, cx| {
                        let _ = popup.update(cx, |p, cx| p.dismiss(w, cx));
                        on_select(locale, w, cx);
                    })
            }))
    })
}

/// Options / Settings page language picker section shared across Word, Deck, and Sheet.
pub fn language_options_section<F>(
    current: UiLocale,
    palette: Palette,
    max_width: f32,
    on_select: F,
) -> Div
where
    F: Fn(UiLocale, &mut Window, &mut App) + Clone + 'static,
{
    div()
        .flex()
        .flex_col()
        .gap_2()
        .max_w(px(max_width))
        .child(
            div()
                .text_size(px(15.))
                .font_weight(FontWeight::SEMIBOLD)
                .child(shared_text(current, "language.section")),
        )
        .child(
            div()
                .text_size(px(12.))
                .text_color(rgb(palette.muted))
                .child(shared_text(current, "language.description")),
        )
        .child(
            div()
                .id("joyoffice-language-options")
                .flex()
                .flex_wrap()
                .gap_2()
                .max_h(px(260.))
                .overflow_y_scroll()
                .children(UiLocale::ALL.into_iter().map(|locale| {
                    let selected = locale == current;
                    let on_select = on_select.clone();
                    let aria = format!("{} ({})", locale.native_name(), locale.tag());
                    Button::new(SharedString::from(format!("lang-option-{}", locale.tag())))
                        .ghost()
                        .w(px(164.))
                        .h(px(40.))
                        .px_3()
                        .rounded(px(6.))
                        .border_1()
                        .border_color(rgb(if selected {
                            palette.accent
                        } else {
                            palette.border
                        }))
                        .bg(rgb(if selected {
                            palette.selected
                        } else {
                            palette.surface
                        }))
                        .selected(selected)
                        .toggled(selected)
                        .accessibility_label(aria)
                        .child(
                            div()
                                .w_full()
                                .flex()
                                .items_center()
                                .justify_between()
                                .gap_2()
                                .child(
                                    div().flex().items_center().gap_1().child(
                                        div()
                                            .text_size(px(13.))
                                            .when(selected, |el| {
                                                el.font_weight(FontWeight::SEMIBOLD)
                                                    .text_color(rgb(palette.accent))
                                            })
                                            .child(locale.native_name()),
                                    ),
                                )
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap_1()
                                        .child(
                                            div()
                                                .text_size(px(11.))
                                                .text_color(rgb(palette.muted))
                                                .child(locale.tag()),
                                        )
                                        .when(selected, |el| {
                                            el.child(
                                                Icon::new(IconName::Check)
                                                    .size(px(13.))
                                                    .text_color(rgb(palette.accent)),
                                            )
                                        }),
                                ),
                        )
                        .on_click(move |_, w, cx| on_select(locale, w, cx))
                })),
        )
}
