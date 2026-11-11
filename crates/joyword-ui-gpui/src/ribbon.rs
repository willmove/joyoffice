//! Word desktop ribbon presentation and document/selection targeting.
use super::*;
use gpui::component::Icon;
use joyoffice_ui::IconName;
use joyword_shell::editing::DocPos;

#[derive(Clone, PartialEq)]
pub(super) struct Target {
    document: u64,
    revision: u64,
    presentation: u64,
    anchor: DocPos,
    focus: DocPos,
}

#[derive(Clone, Copy)]
pub(super) struct Tool {
    zh: &'static str,
    en: &'static str,
    icon: IconName,
    layout: ToolLayout,
    width: Option<f32>,
    tint: Option<u32>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ToolLayout {
    Large,
    Tile,
    Split,
    Compact,
    Icon,
}

impl Tool {
    pub(super) fn label(&self, shell: &Shell) -> &'static str {
        shell.tr(self.zh, self.en)
    }

    pub(super) fn width(mut self, width: f32) -> Self {
        self.width = Some(width);
        self
    }
    pub(super) fn tint(mut self, color: u32) -> Self {
        self.tint = Some(color);
        self
    }
    pub(super) fn new(zh: &'static str, en: &'static str, icon: IconName) -> Self {
        Self {
            zh,
            en,
            icon,
            layout: ToolLayout::Large,
            width: None,
            tint: None,
        }
    }
    pub(super) fn compact(zh: &'static str, en: &'static str, icon: IconName) -> Self {
        Self {
            layout: ToolLayout::Compact,
            ..Self::new(zh, en, icon)
        }
    }
    pub(super) fn tile(zh: &'static str, en: &'static str, icon: IconName) -> Self {
        Self {
            layout: ToolLayout::Tile,
            ..Self::new(zh, en, icon)
        }
    }
    pub(super) fn icon(zh: &'static str, en: &'static str, icon: IconName) -> Self {
        Self {
            layout: ToolLayout::Icon,
            ..Self::new(zh, en, icon)
        }
    }
    pub(super) fn split(mut self) -> Self {
        self.layout = ToolLayout::Split;
        self
    }
}

impl JoyWord {
    pub(super) fn ribbon_group(
        &self,
        id: &'static str,
        label: &'static str,
        body: impl IntoElement,
    ) -> gpui::Stateful<gpui::Div> {
        self.group_box(label, div().flex().items_center().h(px(72.)).child(body))
            .id(id)
            .flex_shrink_0()
            .role(gpui::Role::Group)
            .aria_label(label)
    }

    pub(super) fn ribbon_group_with_launcher(
        &self,
        id: &'static str,
        title: &'static str,
        body: impl IntoElement,
        launcher: Option<Button>,
    ) -> gpui::Stateful<gpui::Div> {
        self.group_frame(
            div().flex().items_center().h(px(72.)).child(body),
            div()
                .relative()
                .flex()
                .items_center()
                .justify_center()
                .h(px(18.))
                .text_size(px(11.))
                .text_color(rgb(self.palette().muted))
                .child(title)
                .children(launcher.map(|button| button.absolute().right_0().bottom_0())),
        )
        .id(id)
        .flex_shrink_0()
        .role(gpui::Role::Group)
        .aria_label(title)
    }

    pub(super) fn ribbon_centered_group(
        &self,
        id: &'static str,
        title: &'static str,
        body: impl IntoElement,
    ) -> gpui::Stateful<gpui::Div> {
        self.ribbon_group_with_launcher(id, title, body, None)
            .px(px(6.))
    }

    pub(super) fn ribbon_target(&self) -> Option<Target> {
        let r = self.canvas.reader.as_ref()?;
        Some(Target {
            document: r.engine().document_id(),
            revision: r.engine().rev(),
            presentation: r.presentation_revision(),
            anchor: r.engine().sel.anchor.clone(),
            focus: r.engine().sel.focus.clone(),
        })
    }
    pub(super) fn ribbon_tool(
        &self,
        id: impl Into<gpui::ElementId>,
        tool: Tool,
        enabled: bool,
        menu: bool,
    ) -> Button {
        let p = self.palette();
        let label = tool.label(&self.shell);
        let compact = matches!(tool.layout, ToolLayout::Compact | ToolLayout::Icon);
        let width = tool.width.unwrap_or(if compact { 100. } else { 56. });
        let (height, icon_size) = match tool.layout {
            ToolLayout::Large => (72., 28.),
            ToolLayout::Tile => (72., 28.),
            ToolLayout::Split => (48., 24.),
            ToolLayout::Compact | ToolLayout::Icon => (21., 15.),
        };
        let icon_color = if !enabled {
            p.disabled
        } else if compact {
            p.text
        } else {
            tool.tint.unwrap_or(p.accent)
        };
        let icon = Icon::new(tool.icon)
            .size(px(icon_size))
            .text_color(rgb(icon_color));
        let caption = div()
            .text_size(px(if compact { 11. } else { 12. }))
            .line_height(px(if tool.layout == ToolLayout::Tile {
                13.
            } else {
                14.
            }))
            .child(label);
        let caret = || Icon::new(IconName::ChevronDown).size(px(10.));
        let content = if compact {
            div()
                .flex()
                .items_center()
                .gap(px(5.))
                .w_full()
                .child(icon)
                .when(tool.layout != ToolLayout::Icon, |v| {
                    v.child(caption.whitespace_nowrap())
                })
                .when(menu, |el| el.child(caret()))
        } else {
            div()
                .flex()
                .flex_col()
                .items_center()
                .h(px(height
                    - if tool.layout == ToolLayout::Large {
                        6.
                    } else {
                        2.
                    }))
                .gap(px(if tool.layout == ToolLayout::Large {
                    4.
                } else {
                    2.
                }))
                .child(
                    div()
                        .flex()
                        .h(px(icon_size))
                        .items_center()
                        .gap(px(2.))
                        .child(icon),
                )
                .child(
                    caption
                        .max_w(px(width - 4.))
                        .whitespace_normal()
                        .text_center(),
                )
                .when(
                    menu && matches!(tool.layout, ToolLayout::Large | ToolLayout::Tile),
                    |el| el.child(caret()),
                )
        };
        Button::new(id)
            .ghost()
            .xsmall()
            .p_0()
            .h(px(height))
            .w(px(width))
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

impl JoyWord {
    pub(super) fn ribbon_tab_visible(&self, index: Option<usize>) -> bool {
        match index {
            Some(9) => self.developer_visible,
            Some(6 | 13) => self
                .canvas
                .reader
                .as_ref()
                .is_some_and(|r| r.table_info().is_some()),
            Some(7) => self
                .canvas
                .reader
                .as_ref()
                .is_some_and(|r| r.picture_info().is_some()),
            _ => true,
        }
    }

    pub(super) fn sync_contextual_tab(&mut self) {
        if !self.ribbon_tab_visible(Some(self.tab)) {
            self.tab = 0;
        }
    }
}
