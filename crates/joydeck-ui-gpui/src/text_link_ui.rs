//! Edit the selected text's click link through the document's normal text transaction.
use super::*;
use gpui_kit::component::{Sizable, button::ButtonVariants};
use joydeck_engine::text::links;
use joydeck_model::{
    Presentation,
    text::{Action, Hyperlink, safe_url},
};

fn address(action: &Action, document: &Presentation) -> Option<String> {
    match action {
        Action::Url { url } => Some(url.clone()),
        Action::Slide { slide } => Some(format!("#{}", document.slide_index(*slide)? + 1)),
        _ => None,
    }
}

fn parse_address(
    value: &str,
    document: &Presentation,
    locale: joyoffice_ui::UiLocale,
) -> Result<Action, String> {
    let value = value.trim();
    if let Some(page) = value.strip_prefix('#') {
        let index = page.parse::<usize>().ok().and_then(|n| n.checked_sub(1));
        let slide = index
            .and_then(|index| document.slides.get(index))
            .ok_or_else(|| crate::l10n::t(locale, "link.err_page").to_owned())?;
        return Ok(Action::Slide { slide: slide.id });
    }
    if value.len() > 2048
        || value.chars().any(|c| c.is_control() || c.is_whitespace())
        || !safe_url(value)
        || url::Url::parse(value).is_err()
    {
        return Err(crate::l10n::t(locale, "link.err_url").into());
    }
    Ok(Action::Url { url: value.into() })
}

impl Viewer {
    pub(super) fn text_link_controls(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let disabled = self.loading
            || self.composing()
            || self.benchmark.is_some()
            || self.format_targets().len() != 1;
        let locale = self.locale;
        let view = cx.entity().downgrade();
        chrome::row()
            .gap_0()
            .child(
                chrome::tool(
                    "insert-text-link",
                    joyoffice_ui::IconName::Link,
                    self.t("link.btn"),
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
                            .accessibility_label(self.t("link.options_aria"))
                            .disabled(disabled),
                    )
                    .content(move |_, _, cx| {
                        let popup = cx.entity().downgrade();
                        let view = view.clone();
                        Button::new("remove-text-link")
                            .ghost()
                            .xsmall()
                            .label(crate::l10n::t(locale, "link.remove"))
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
            self.message = self.t("link.err_select_text").into();
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
                self.message = self.t("link.err_other_action").into();
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
        let mut fields = vec![
            (self.t("link.address_field"), original.clone()),
            (self.t("link.tooltip_field"), tooltip),
        ];
        if let Some(label) = &label {
            fields.push((self.t("link.label_field"), label.clone()));
        }
        self.fields_dialog(
            self.t("link.dialog_title"),
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
                    return Err(s.t("link.err_selection_changed").into());
                }
                let editor = s
                    .editor
                    .as_ref()
                    .ok_or_else(|| s.t("link.err_doc_closed").to_owned())?;
                if editor.session.revision() != revision {
                    return Err(s.t("dialog.doc_changed").into());
                }
                let document = editor.session.model();
                let action = if values[0] == original
                    && let Some(link) = &scope.common
                {
                    link.action.clone()
                } else {
                    parse_address(&values[0], document, s.locale)?
                };
                if values[1].len() > 1024 || values[1].chars().any(char::is_control) {
                    return Err(s.t("link.err_tooltip_invalid").into());
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
                    return Err(s.t("link.err_label_empty").into());
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
        let locale = self.locale;
        let i = self
            .inline
            .as_mut()
            .ok_or_else(|| crate::l10n::t(locale, "link.err_selection_closed").to_owned())?;
        let mut draft = i.draft.clone();
        let range = if let Some(label) = replacement {
            if label.trim().is_empty() || label.len() > 4096 || label.chars().any(char::is_control)
            {
                return Err(crate::l10n::t(locale, "link.err_label_invalid").into());
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
