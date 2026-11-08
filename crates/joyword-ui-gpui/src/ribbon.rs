//! Word desktop ribbon presentation and document/selection targeting.
use super::*;
use gpui::component::Icon;
use joyoffice_ui::IconName;

#[derive(Clone, Copy)]
pub(super) struct Tool {
    zh: &'static str,
    en: &'static str,
    icon: IconName,
    compact: bool,
    width: Option<f32>,
    caption_width: Option<f32>,
}

impl Tool {
    pub(super) fn caption_width(mut self, width: f32) -> Self {
        self.caption_width = Some(width);
        self
    }
    pub(super) fn width(mut self, width: f32) -> Self {
        self.width = Some(width);
        self
    }
    pub(super) fn new(zh: &'static str, en: &'static str, icon: IconName) -> Self {
        Self {
            zh,
            en,
            icon,
            compact: false,
            width: None,
            caption_width: None,
        }
    }
    pub(super) fn compact(zh: &'static str, en: &'static str, icon: IconName) -> Self {
        Self {
            compact: true,
            ..Self::new(zh, en, icon)
        }
    }
}

impl JoyWord {
    pub(super) fn ribbon_tool(
        &self,
        id: impl Into<gpui::ElementId>,
        tool: Tool,
        enabled: bool,
        menu: bool,
    ) -> Button {
        let p = self.palette();
        let label = self.shell.tr(tool.zh, tool.en);
        let icon = Icon::new(tool.icon)
            .size(px(if tool.compact { 15. } else { 28. }))
            .text_color(rgb(if !enabled {
                p.disabled
            } else {
                match tool.icon {
                    IconName::Image | IconName::Shapes | IconName::Box | IconName::Type => p.accent,
                    IconName::MessageSquarePlus | IconName::Network => {
                        if self.shell.dark {
                            0x88c99c
                        } else {
                            0x3a8055
                        }
                    }
                    _ => p.text,
                }
            }));
        let caption = div()
            .text_size(px(11.))
            .line_height(px(13.))
            .max_w(px(tool.caption_width.unwrap_or_else(|| {
                if tool.compact {
                    tool.width.map_or(72., |w| w - 36.)
                } else {
                    tool.width.map_or(52., |w| (w - 4.).max(52.))
                }
            })))
            .whitespace_normal()
            .text_center()
            .child(label);
        let caret = || Icon::new(IconName::ChevronDown).size(px(10.));
        let content = if tool.compact {
            div()
                .flex()
                .items_center()
                .gap(px(5.))
                .w_full()
                .child(icon)
                .child(caption)
                .when(menu, |el| el.child(caret()))
        } else {
            div()
                .flex()
                .flex_col()
                .items_center()
                .gap(px(4.))
                .child(
                    div()
                        .flex()
                        .h(px(28.))
                        .items_center()
                        .gap(px(2.))
                        .child(icon)
                        .when(menu, |el| el.child(caret())),
                )
                .child(caption)
        };
        Button::new(id)
            .ghost()
            .xsmall()
            .p_0()
            .h(px(if tool.compact { 21. } else { 64. }))
            .w(px(tool.width.unwrap_or(if tool.compact {
                100.
            } else {
                match tool.icon {
                    IconName::Sticker | IconName::Box | IconName::Radical | IconName::Omega => 44.,
                    IconName::Hash => 36.,
                    _ => 52.,
                }
            })))
            .flex_shrink_0()
            .accessibility_label(label)
            .tooltip(if enabled {
                label
            } else {
                self.shell.tr(
                    "此功能尚未提供，或当前文档状态不可用",
                    "Not available in this document or build",
                )
            })
            .disabled(!enabled)
            .child(content)
    }
}
