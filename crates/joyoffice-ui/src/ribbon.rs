//! Accessible ribbon groups shared by the desktop applications.
use crate::Palette;
use gpui_kit::{
    Div, ElementId, Role, SharedString, Stateful, component::button::Button, div, prelude::*, px,
    rgb,
};

/// Give an existing group its stable control ID and its localized accessible name.
pub fn ribbon_group(
    frame: Div,
    id: impl Into<ElementId>,
    title: impl Into<SharedString>,
) -> Stateful<Div> {
    frame.id(id).role(Role::Group).aria_label(title)
}

/// Center the group name, keeping the optional, product-bound launcher at the trailing edge.
pub fn ribbon_group_footer(
    palette: Palette,
    title: impl Into<SharedString>,
    launcher: Option<Button>,
) -> Div {
    div()
        .relative()
        .flex()
        .items_center()
        .justify_center()
        .h(px(18.))
        .text_size(px(11.))
        .text_color(rgb(palette.muted))
        .child(title.into())
        .children(launcher.map(|button| button.absolute().right_0()))
}
