//! Project the shared palette into both custom chrome and GPUI Kit controls.
use crate::{Brand, Palette, ThemePreset, metrics};
use gpui_kit::{
    component::{Theme, ThemeMode},
    *,
};

#[derive(Clone, Copy)]
pub struct AppliedTheme(pub ThemePreset);
impl Global for AppliedTheme {}

pub fn apply_theme(preset: ThemePreset, brand: Brand, window: Option<&mut Window>, cx: &mut App) {
    cx.set_global(AppliedTheme(preset));
    Theme::change(
        if preset.is_dark() {
            ThemeMode::Dark
        } else {
            ThemeMode::Light
        },
        window,
        cx,
    );
    let p = preset.palette(brand);
    // update reconciles component tokens and the Base projection, then refreshes
    // every window. Editing colors alone leaves inputs/popovers on stale tokens.
    Theme::update(cx, |theme| {
        let c = &mut theme.colors;
        c.background = rgb(p.chrome).into();
        c.foreground = rgb(p.text).into();
        c.border = rgb(p.border).into();
        c.input = rgb(p.border).into();
        c.primary = rgb(p.accent).into();
        c.primary_hover = rgb(p.accent_hover).into();
        c.primary_active = rgb(p.accent_hover).into();
        c.primary_foreground = rgb(p.on_accent).into();
        c.ring = rgb(p.accent).into();
        c.caret = rgb(p.accent).into();
        c.accent = rgb(p.selected).into();
        c.accent_foreground = rgb(p.text).into();
        c.muted = rgb(p.chrome).into();
        c.muted_foreground = rgb(p.muted).into();
        c.popover = rgb(p.surface).into();
        c.popover_foreground = rgb(p.text).into();
        c.button = rgb(p.surface).into();
        c.button_hover = rgb(p.hover).into();
        c.button_active = rgb(p.pressed).into();
        c.button_foreground = rgb(p.text).into();
        c.button_primary = c.primary;
        c.button_primary_hover = c.primary_hover;
        c.button_primary_active = c.primary_active;
        c.button_primary_foreground = c.primary_foreground;
        c.secondary = rgb(p.chrome).into();
        c.secondary_hover = rgb(p.hover).into();
        c.secondary_active = rgb(p.pressed).into();
        c.secondary_foreground = rgb(p.text).into();
        c.button_secondary = c.secondary;
        c.button_secondary_hover = c.secondary_hover;
        c.button_secondary_active = c.secondary_active;
        c.button_secondary_foreground = c.secondary_foreground;
        c.list = rgb(p.surface).into();
        c.list_head = rgb(p.chrome).into();
        c.list_even = rgb(p.chrome).into();
        c.list_hover = rgb(p.hover).into();
        c.list_active = rgb(p.selected).into();
        c.list_active_border = rgb(p.accent).into();
        c.sidebar = rgb(p.chrome).into();
        c.sidebar_foreground = rgb(p.text).into();
        c.sidebar_accent = rgb(p.selected).into();
        c.sidebar_accent_foreground = rgb(p.text).into();
        c.sidebar_border = rgb(p.border).into();
        c.sidebar_primary = c.primary;
        c.sidebar_primary_foreground = c.primary_foreground;
        c.tab = rgb(p.chrome).into();
        c.tab_bar = rgb(p.chrome).into();
        c.tab_active = rgb(p.surface).into();
        c.tab_foreground = rgb(p.muted).into();
        c.tab_active_foreground = rgb(p.text).into();
        c.title_bar = rgb(p.header).into();
        c.title_bar_border = rgb(p.border).into();
        c.selection = rgb(p.accent).opacity(0.25).into();
        c.link = rgb(p.accent).into();
        c.link_hover = rgb(p.accent_hover).into();
        c.link_active = c.link_hover;
        c.group_box = rgb(p.surface).into();
        c.group_box_foreground = rgb(p.text).into();
        theme.radius = px(metrics::RADIUS);
    });
}

/// Preview content only; callers supply identity, activation and selected state.
pub fn theme_preview(preset: ThemePreset, brand: Brand, chinese: bool) -> Div {
    let p: Palette = preset.palette(brand);
    div()
        .flex()
        .flex_col()
        .gap_2()
        .w_full()
        .p_2()
        .text_size(px(12.))
        .child(
            div()
                .flex()
                .h(px(30.))
                .w_full()
                .rounded(px(3.))
                .overflow_hidden()
                .bg(rgb(p.canvas))
                .border_1()
                .border_color(rgb(p.border))
                .child(div().w(px(22.)).h_full().bg(rgb(p.header)))
                .child(
                    div()
                        .flex_1()
                        .m_1()
                        .bg(rgb(p.surface))
                        .border_l(px(3.))
                        .border_color(rgb(p.accent)),
                )
                .child(div().w(px(15.)).h_full().bg(rgb(p.accent))),
        )
        .child(preset.label(chinese))
}
