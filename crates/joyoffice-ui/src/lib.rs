//! Product-neutral presentation primitives. No editor, command or persistence state.
//!
//! Applications own identity, accessibility, focus routing and activation. These
//! surfaces only supply the shared visual vocabulary and interaction states.
pub use gpui_kit::assets::IconName;
use gpui_kit::component::Icon;
use gpui_kit::{Div, ParentElement, Styled, div, px, rgb};

pub use joyoffice_theme::{Brand, Palette, ThemePreset};
mod appearance;
pub use appearance::{AppliedTheme, apply_theme, theme_preview};

/// Logical pixels; document zoom never changes application chrome dimensions.
pub mod metrics {
    pub const CONTROL: f32 = 30.0;
    pub const HEADER: f32 = 48.0;
    pub const TAB: f32 = 36.0;
    pub const STATUS: f32 = 32.0;
    pub const ICON: f32 = 18.0;
    pub const RADIUS: f32 = 6.0;
    pub const CARD_RADIUS: f32 = 10.0;
}

pub fn control(palette: Palette, enabled: bool, selected: bool) -> Div {
    div()
        .flex()
        .items_center()
        .justify_center()
        .flex_shrink_0()
        .h(px(metrics::CONTROL))
        .px_2()
        .gap_2()
        .rounded(px(metrics::RADIUS))
        .border_1()
        .border_color(if selected {
            rgb(palette.accent).into()
        } else {
            gpui_kit::transparent_black()
        })
        .bg(if selected {
            rgb(palette.selected).into()
        } else {
            gpui_kit::transparent_black()
        })
        .text_color(rgb(if !enabled {
            palette.disabled
        } else if selected {
            palette.accent
        } else {
            palette.text
        }))
        .text_size(px(13.0))
}

pub fn card(palette: Palette) -> Div {
    div()
        .bg(rgb(palette.surface))
        .text_color(rgb(palette.text))
        .border_1()
        .border_color(rgb(palette.border))
        .rounded(px(metrics::CARD_RADIUS))
}

/// A section heading inside a task pane or form. Identity stays with the caller.
pub fn section(palette: Palette, title: impl Into<gpui_kit::SharedString>) -> Div {
    div()
        .pt_3()
        .pb_1()
        .text_size(px(12.0))
        .font_weight(gpui_kit::FontWeight::SEMIBOLD)
        .text_color(rgb(palette.text))
        .child(title.into())
}

/// Keep labels and their controls together when a form reflows or scrolls.
pub fn field(palette: Palette, label: impl Into<gpui_kit::SharedString>) -> Div {
    div()
        .flex()
        .flex_col()
        .gap_1()
        .min_w_0()
        .text_size(px(12.0))
        .text_color(rgb(palette.muted))
        .child(label.into())
}

pub fn icon(name: IconName, size: f32, color: u32) -> Icon {
    Icon::new(name).size(px(size)).text_color(rgb(color))
}
