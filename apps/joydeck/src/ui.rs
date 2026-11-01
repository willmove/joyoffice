//! Deck's adapter for the product-neutral JoyOffice visual vocabulary.
use super::*;
use gpui_kit::{
    assets::IconName as I,
    component::{Sizable, button::ButtonVariants},
};
pub(super) const PALETTE: joyoffice_ui::Palette =
    joyoffice_ui::Palette::light(joyoffice_ui::Brand::Deck);
pub(super) const ACCENT: u32 = PALETTE.accent;
pub(super) fn label(text: impl Into<SharedString>) -> Div {
    div()
        .text_size(px(12.))
        .text_color(rgb(PALETTE.muted))
        .child(text.into())
}
pub(super) fn caption_chrome(text: &str) -> Div {
    label(text.to_owned())
}
pub(super) fn group(name: &str, content: impl IntoElement) -> Div {
    div()
        .flex_shrink_0()
        .flex()
        .flex_col()
        .justify_between()
        .h(px(80.))
        .px_3()
        .border_r_1()
        .border_color(rgb(PALETTE.separator))
        .child(div().h(px(62.)).child(content))
        .child(label(name.to_owned()).text_size(px(11.)))
}
pub(super) fn tool(id: &'static str, name: &'static str, icon: I) -> Button {
    let accessible = if name.is_empty() {
        match id {
            "save" => "保存",
            "undo" => "撤销",
            "redo" => "重做",
            "rail-add" => "新建幻灯片",
            _ => id,
        }
    } else {
        name
    };
    Button::new(id)
        .accessibility_id(id)
        .ghost()
        .small()
        .h(px(joyoffice_ui::metrics::CONTROL))
        .rounded(px(joyoffice_ui::metrics::RADIUS))
        .icon(icon)
        .label(name)
        .accessibility_label(accessible)
        .tooltip(accessible)
}
pub(super) fn large_tool(id: &'static str, name: &'static str, icon: I) -> Button {
    Button::new(id)
        .accessibility_id(id)
        .ghost()
        .h(px(62.))
        .px_3()
        .rounded(px(joyoffice_ui::metrics::RADIUS))
        .accessibility_label(name)
        .tooltip(name)
        .child(
            div()
                .flex()
                .flex_col()
                .items_center()
                .gap_2()
                .child(
                    gpui_kit::component::Icon::from(icon)
                        .size(px(24.))
                        .text_color(rgb(ACCENT)),
                )
                .child(div().text_size(px(12.)).child(name)),
        )
}
pub(super) fn section(text: &str) -> Div {
    joyoffice_ui::section(PALETTE, text.to_owned())
}
pub(super) fn field(text: &str, input: &Entity<InputState>) -> Div {
    joyoffice_ui::field(PALETTE, text.to_owned()).child(
        Input::new(input)
            .aria_label(text.to_owned())
            .h(px(joyoffice_ui::metrics::CONTROL)),
    )
}
pub(super) fn swatch(value: &str) -> Div {
    let color = value
        .strip_prefix('#')
        .and_then(|s| u32::from_str_radix(s, 16).ok())
        .filter(|_| value.len() == 7);
    div()
        .w(px(18.))
        .h(px(18.))
        .flex_shrink_0()
        .rounded_sm()
        .border_1()
        .border_color(rgb(PALETTE.border))
        .bg(rgb(color.unwrap_or(PALETTE.surface)))
        .children(color.is_none().then(|| div().text_xs().child("?")))
}

pub(super) fn pane(width: f32) -> Div {
    div()
        .w(px(width))
        .h_full()
        .flex_shrink_0()
        .flex()
        .flex_col()
        .overflow_hidden()
        .border_l_1()
        .border_color(rgb(PALETTE.border))
        .bg(rgb(PALETTE.surface))
}
impl Editor {
    pub(super) fn pane_header(&self, title: &str, cx: &mut Context<Self>) -> Div {
        div()
            .h(px(48.))
            .flex_shrink_0()
            .px_4()
            .flex()
            .items_center()
            .justify_between()
            .child(
                div()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(title.to_owned()),
            )
            .child(
                tool("inspector-close", "", I::X)
                    .accessibility_label("关闭属性面板")
                    .tooltip("关闭属性面板")
                    .on_click(cx.listener(|s, _, w, cx| {
                        s.inspector_open = false;
                        s.select_ribbon("开始", w, cx);
                    })),
            )
    }
}
