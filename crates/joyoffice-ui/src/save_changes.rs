//! Presentation of the desktop save prompt. The caller binds its own actions and focus.
use crate::Palette;
use gpui_kit::{
    AnyElement, Div, FontWeight, Role, SharedString, Stateful, div, prelude::*, px, rgb, rgba,
};

pub fn save_changes_backdrop() -> Div {
    div()
        .absolute()
        .inset_0()
        .occlude()
        .bg(rgba(0x0e1e1855))
        .flex()
        .items_center()
        .justify_center()
}

/// Actions appear in keyboard order: cancel, alternate (discard or save a copy), save.
pub fn save_changes_panel(
    palette: Palette,
    title: impl Into<SharedString>,
    message: impl Into<SharedString>,
    status: Option<AnyElement>,
    actions: [AnyElement; 3],
) -> Stateful<Div> {
    let title = title.into();
    div()
        .id("save-changes-dialog")
        .debug_selector(|| "save-changes-dialog".into())
        .role(Role::Dialog)
        .aria_label(title.clone())
        .relative()
        .w(px(450.))
        .max_w_full()
        .m_4()
        .p_6()
        .rounded_xl()
        .bg(rgb(palette.surface))
        .shadow_xl()
        .flex()
        .flex_col()
        .gap_4()
        .child(
            div()
                .text_lg()
                .font_weight(FontWeight::SEMIBOLD)
                .child(title),
        )
        .child(div().whitespace_normal().child(message.into()))
        .children(status)
        .child(div().flex().justify_end().gap_2().children(actions))
}
