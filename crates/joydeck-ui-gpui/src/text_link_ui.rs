//! Edit the selected text's click link through the document's normal text transaction.
use super::*;
use gpui_kit::component::{Sizable, button::ButtonVariants};
use joydeck_engine::text::links;
use joydeck_model::{
    Presentation,
    text::{Action, Hyperlink, safe_url},
};

const ADDRESS: &str = "链接地址（网页、邮件或 #页码）";
const TOOLTIP: &str = "屏幕提示（可选）";
const LABEL: &str = "显示文字";

fn address(action: &Action, document: &Presentation) -> Option<String> {
    match action {
        Action::Url { url } => Some(url.clone()),
        Action::Slide { slide } => Some(format!("#{}", document.slide_index(*slide)? + 1)),
        _ => None,
    }
}

fn parse_address(value: &str, document: &Presentation) -> Result<Action, String> {
    let value = value.trim();
    if let Some(page) = value.strip_prefix('#') {
        let index = page.parse::<usize>().ok().and_then(|n| n.checked_sub(1));
        let slide = index
            .and_then(|index| document.slides.get(index))
            .ok_or("请输入有效页码，如 #2")?;
        return Ok(Action::Slide { slide: slide.id });
    }
    if value.len() > 2048
        || value.chars().any(|c| c.is_control() || c.is_whitespace())
        || !safe_url(value)
        || url::Url::parse(value).is_err()
    {
        return Err("请输入 HTTP／HTTPS、mailto 地址或 #页码".into());
    }
    Ok(Action::Url { url: value.into() })
}

impl Viewer {
    pub(super) fn text_link_controls(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let disabled = self.loading
            || self.composing()
            || self.benchmark.is_some()
            || self.format_targets().len() != 1;
        let view = cx.entity().downgrade();
        chrome::row()
            .gap_0()
            .child(
                chrome::tool(
                    "insert-text-link",
                    joyoffice_ui::IconName::Link,
                    "链接",
                    0x326797,
                )
                .disabled(disabled)
                .on_click(cx.listener(|s, _, w, cx| s.edit_text_link(&EditTextLink, w, cx))),
            )
            .child(
                gpui_kit::component::popover::Popover::new("link-options")
                    .trigger(
                        Button::new("link-options-trigger")
                            .ghost()
                            .xsmall()
                            .p_0()
                            .w(px(14.))
                            .h(px(35.))
                            .icon(joyoffice_ui::IconName::ChevronDown)
                            .accessibility_label("链接选项")
                            .disabled(disabled),
                    )
                    .content(move |_, _, cx| {
                        let popup = cx.entity().downgrade();
                        let view = view.clone();
                        Button::new("remove-text-link")
                            .ghost()
                            .xsmall()
                            .label("移除链接")
                            .on_click(move |_, w, cx| {
                                let _ = popup.update(cx, |p, cx| p.dismiss(w, cx));
                                let _ = view.update(cx, |s, cx| s.remove_text_link(w, cx));
                            })
                    }),
            )
    }

    fn link_scope(
        &mut self,
        insert: bool,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<links::Selection> {
        if self.loading || self.composing() || self.benchmark.is_some() {
            return None;
        }
        if self.inline.is_none() && self.format_targets().len() == 1 {
            self.begin_selected_text(w, cx);
        }
        let scope = self.inline.as_ref().and_then(|i| {
            links::selection(&i.draft.body, i.draft.selection.clone()).or_else(|| {
                (insert && i.draft.selection.is_empty()).then(|| links::Selection {
                    range: i.draft.selection.clone(),
                    common: None,
                    has_link: false,
                })
            })
        });
        if scope.is_none() {
            self.message = "请先选择需要链接的文字".into();
            cx.notify();
        }
        scope
    }

    pub(super) fn edit_text_link(
        &mut self,
        _: &EditTextLink,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(scope) = self.link_scope(true, w, cx) else {
            return;
        };
        let (Some(i), Some(editor)) = (&self.inline, &self.editor) else {
            return;
        };
        let mut original = String::new();
        let mut tooltip = String::new();
        if let Some(link) = &scope.common {
            let Some(initial) = address(&link.action, editor.session.model()) else {
                self.message = "当前链接使用其他动作，可移除后重新设置".into();
                cx.notify();
                return;
            };
            original = initial;
            tooltip.clone_from(&link.tooltip);
        }
        let target = i.target.clone();
        let selection = i.draft.selection.clone();
        let page = self.page;
        let revision = editor.session.revision();
        let label = links::plain_label(&i.draft.body, scope.range.clone());
        let mut fields = vec![(ADDRESS, original.clone()), (TOOLTIP, tooltip)];
        if let Some(label) = &label {
            fields.push((LABEL, label.clone()));
        }
        self.fields_dialog(
            "编辑文字链接",
            fields,
            w,
            cx,
            move |s, values, w, cx| {
                if s.page != page
                    || s.loading
                    || s.composing()
                    || s.benchmark.is_some()
                    || !s
                        .inline
                        .as_ref()
                        .is_some_and(|i| i.target == target && i.draft.selection == selection)
                {
                    return Err("文字选区已变化，请重新打开此对话框".into());
                }
                let editor = s.editor.as_ref().ok_or("文稿已关闭")?;
                if editor.session.revision() != revision {
                    return Err("文稿已变化，请重新打开此对话框".into());
                }
                let document = editor.session.model();
                let action = if values[0] == original
                    && let Some(link) = &scope.common
                {
                    link.action.clone()
                } else {
                    parse_address(&values[0], document)?
                };
                if values[1].len() > 1024 || values[1].chars().any(char::is_control) {
                    return Err("屏幕提示超出长度或包含控制字符".into());
                }
                let mut link = scope.common.clone().unwrap_or(Hyperlink {
                    action: action.clone(),
                    tooltip: String::new(),
                    highlight_click: false,
                    sound: None,
                    stop_sound: false,
                    zoom_return: None,
                });
                if link.action != action {
                    link.zoom_return = None;
                }
                link.action = action;
                link.tooltip.clone_from(&values[1]);
                let replacement = values.get(2).filter(|value| label.as_ref() != Some(value));
                if scope.range.is_empty() && replacement.is_none() {
                    return Err("请输入显示文字".into());
                }
                s.apply_text_link(
                    scope.range.clone(),
                    Some(&link),
                    replacement.map(String::as_str),
                    w,
                    cx,
                )
            },
        );
    }

    fn remove_text_link(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        if let Some(scope) = self.link_scope(false, w, cx)
            && scope.has_link
            && let Err(error) = self.apply_text_link(scope.range, None, None, w, cx)
        {
            self.message = error;
            cx.notify();
        }
    }

    fn apply_text_link(
        &mut self,
        range: std::ops::Range<usize>,
        link: Option<&Hyperlink>,
        replacement: Option<&str>,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        let i = self.inline.as_mut().ok_or("文字选区已关闭")?;
        let mut draft = i.draft.clone();
        let range = if let Some(label) = replacement {
            if label.trim().is_empty() || label.len() > 4096 || label.chars().any(char::is_control)
            {
                return Err("显示文字不能为空、超过 4096 字节或包含控制字符".into());
            }
            let start = range.start;
            draft.selection = range;
            draft.typing_props = None;
            draft.replace(None, label, false, None)?;
            start..draft.cursor()
        } else {
            range
        };
        let after = links::apply(&draft.body, range, link)?;
        if after != i.draft.body {
            draft.body = after;
            draft.typing_props = None;
            i.draft = draft;
            self.publish_inline(w, cx);
        }
        Ok(())
    }
}
