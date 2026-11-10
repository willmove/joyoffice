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
    compact: bool,
    width: Option<f32>,
}

impl Tool {
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
    pub(super) fn ribbon_group(
        &self,
        id: &'static str,
        label: &'static str,
        body: impl IntoElement,
    ) -> gpui::Stateful<gpui::Div> {
        self.group_box(label, div().flex().items_center().h(px(64.)).child(body))
            .id(id)
            .flex_shrink_0()
            .role(gpui::Role::Group)
            .aria_label(label)
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
        let label = self.shell.tr(tool.zh, tool.en);
        let width = tool.width.unwrap_or(if tool.compact { 100. } else { 56. });
        let icon_size = if tool.compact { 15. } else { 26. };
        let icon_color = if !enabled {
            p.disabled
        } else if tool.compact {
            p.text
        } else {
            p.accent
        };
        let icon = Icon::new(tool.icon)
            .size(px(icon_size))
            .text_color(rgb(icon_color));
        let caption = div()
            .text_size(px(if tool.compact { 11. } else { 12. }))
            .line_height(px(14.))
            .child(label);
        let caret = || Icon::new(IconName::ChevronDown).size(px(10.));
        let content = if tool.compact {
            div()
                .flex()
                .items_center()
                .gap(px(5.))
                .w_full()
                .child(icon)
                .child(caption.whitespace_nowrap())
                .when(menu, |el| el.child(caret()))
        } else {
            div()
                .flex()
                .flex_col()
                .items_center()
                .h(px(58.))
                .gap(px(4.))
                .child(
                    div()
                        .flex()
                        .h(px(icon_size))
                        .items_center()
                        .gap(px(2.))
                        .child(icon)
                        .when(menu, |el| el.child(caret())),
                )
                .child(
                    caption
                        .max_w(px(width - 4.))
                        .whitespace_normal()
                        .text_center(),
                )
        };
        Button::new(id)
            .ghost()
            .xsmall()
            .p_0()
            .h(px(if tool.compact { 21. } else { 64. }))
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
