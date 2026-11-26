//! Insert a native field or fixed date using the ordinary canvas text transaction.
use super::*;
use gpui_kit::component::{Sizable, button::ButtonVariants, popover::Popover};
use joydeck_engine::{Command, existing_footer_summary};
use joydeck_text::datetime::{DATE_KINDS, DateTime, field_text};

const DATE_LABEL_KEYS: [&str; 13] = [
    "field.short_date",
    "field.weekday_date",
    "field.dmy",
    "field.mdy",
    "field.dmy_short",
    "field.month_year",
    "field.short_month_year",
    "field.date_time",
    "field.date_time_sec",
    "field.time_24",
    "field.time_24_sec",
    "field.time_12",
    "field.time_12_sec",
];

fn date_language(index: usize, locale: joyoffice_ui::UiLocale) -> &'static str {
    if index == 0 && locale.is_zh_cn() {
        "zh-CN"
    } else {
        "en-US"
    }
}

#[derive(Clone, Copy)]
enum FooterScope {
    CurrentMissing,
    AllMissing,
    Existing,
    ClearExisting,
}

impl FooterScope {
    fn label(self, locale: joyoffice_ui::UiLocale) -> &'static str {
        let key = match self {
            Self::CurrentMissing => "field.footer_current_missing",
            Self::AllMissing => "field.footer_all_missing",
            Self::Existing => "field.footer_existing",
            Self::ClearExisting => "field.footer_clear_existing",
        };
        crate::l10n::t(locale, key)
    }

    fn command(self, text: String) -> Command {
        match self {
            Self::CurrentMissing => Command::AddMissingFooterText { text },
            Self::AllMissing => Command::AddAllMissingFooterText { text },
            Self::Existing | Self::ClearExisting => Command::SetExistingFooterText { text },
        }
    }
}

impl Viewer {
    #[inline(never)]
    pub(super) fn footer_control(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let view = cx.entity().downgrade();
        let locale = self.locale;
        let disabled = self.editor.is_none() || self.loading || self.composing();
        Popover::new("footer-menu")
            .trigger(
                chrome::tool(
                    "insert-footer",
                    joyoffice_ui::IconName::NotebookText,
                    self.t("insert.header_footer"),
                    0x326797,
                )
                .disabled(disabled),
            )
            .content(move |_, _, cx| {
                let popup = cx.entity().downgrade();
                div().flex().flex_col().gap_1().children(
                    [
                        FooterScope::CurrentMissing,
                        FooterScope::AllMissing,
                        FooterScope::Existing,
                        FooterScope::ClearExisting,
                    ]
                    .into_iter()
                    .enumerate()
                    .map(|(index, scope)| {
                        let popup = popup.clone();
                        let view = view.clone();
                        Button::new(("footer-scope", index))
                            .ghost()
                            .small()
                            .label(scope.label(locale))
                            .on_click(move |_, w, cx| {
                                let _ = popup.update(cx, |p, cx| p.dismiss(w, cx));
                                let _ = view.update(cx, |s, cx| s.footer_dialog(scope, w, cx));
                            })
                    }),
                )
            })
    }

    fn footer_dialog(&mut self, scope: FooterScope, w: &mut Window, cx: &mut Context<Self>) {
        let Some(editor) = &self.editor else { return };
        let page = editor.session.selection().page;
        let fields = if matches!(scope, FooterScope::Existing | FooterScope::ClearExisting) {
            match existing_footer_summary(editor.session.model()) {
                Ok(summary) => {
                    if matches!(scope, FooterScope::ClearExisting) {
                        Vec::new()
                    } else {
                        vec![match summary.text {
                            Some(text) => (self.t("field.footer_text"), text),
                            None => (self.t("field.footer_mixed_prompt"), String::new()),
                        }]
                    }
                }
                Err(error) => {
                    self.message = error;
                    cx.notify();
                    return;
                }
            }
        } else {
            vec![(self.t("field.footer_text"), String::new())]
        };
        self.fields_dialog(
            if matches!(scope, FooterScope::ClearExisting) {
                self.t("field.footer_clear_confirm")
            } else {
                scope.label(self.locale)
            },
            fields,
            w,
            cx,
            move |s, values, w, cx| {
                if matches!(scope, FooterScope::CurrentMissing)
                    && s.editor.as_ref().map(|e| e.session.selection().page) != Some(page)
                {
                    return Err(s.t("field.err_slide_changed").into());
                }
                let text = values.into_iter().next().unwrap_or_default();
                if !matches!(scope, FooterScope::ClearExisting) && text.trim().is_empty() {
                    return Err(s.t("field.err_footer_empty").into());
                }
                if !s.try_dispatch(scope.command(text), w, cx)? {
                    return Err(s.t("field.err_footer_none").into());
                }
                Ok(())
            },
        );
    }

    #[inline(never)]
    pub(super) fn text_field_controls(&self, cx: &mut Context<Self>) -> [AnyElement; 2] {
        let disabled = !self.can_insert_text_field();
        let locale = self.locale;
        let view = cx.entity().downgrade();
        let date = Popover::new("insert-date-menu")
            .trigger(
                chrome::tool(
                    "insert-date",
                    joyoffice_ui::IconName::CalendarClock,
                    self.t("insert.date_time"),
                    0x326797,
                )
                .disabled(disabled),
            )
            .content(move |_, _, cx| {
                let now = DateTime::now_local();
                let popup = cx.entity().downgrade();
                let mut list = div()
                    .id("date-field-types")
                    .flex()
                    .flex_col()
                    .gap_1()
                    .max_h(px(380.))
                    .overflow_y_scroll();
                for (i, (key, kind)) in DATE_LABEL_KEYS.into_iter().zip(DATE_KINDS).enumerate() {
                    let name = crate::l10n::t(locale, key);
                    let lang = date_language(i, locale);
                    let sample = now.and_then(|n| field_text(kind, Some(lang), n));
                    let label = sample
                        .as_ref()
                        .map_or_else(|| name.to_owned(), |sample| format!("{name} · {sample}"));
                    let (dynamic_popup, dynamic_view) = (popup.clone(), view.clone());
                    let (fixed_popup, fixed_view) = (popup.clone(), view.clone());
                    let row = div()
                        .flex()
                        .gap_1()
                        .child(
                            Button::new(("date-field-type", i))
                                .ghost()
                                .xsmall()
                                .label(label)
                                .accessibility_label(name)
                                .disabled(sample.is_none())
                                .on_click(move |_, w, cx| {
                                    let _ = dynamic_popup.update(cx, |p, cx| p.dismiss(w, cx));
                                    let _ = dynamic_view.update(cx, |s, cx| {
                                        s.insert_text_field(kind, Some(lang), None, w, cx)
                                    });
                                }),
                        )
                        .child(
                            Button::new(("fixed-date-type", i))
                                .ghost()
                                .xsmall()
                                .label(crate::l10n::t(locale, "field.fixed"))
                                .accessibility_label(crate::l10n::tf(
                                    locale,
                                    "field.fixed_aria",
                                    &[("name", name)],
                                ))
                                .disabled(sample.is_none())
                                .on_click(move |_, w, cx| {
                                    let Some(value) = sample.as_deref() else {
                                        return;
                                    };
                                    let _ = fixed_popup.update(cx, |p, cx| p.dismiss(w, cx));
                                    let _ = fixed_view.update(cx, |s, cx| {
                                        s.insert_text_field(kind, Some(lang), Some(value), w, cx)
                                    });
                                }),
                        );
                    list = list.child(row);
                }
                list
            });
        [
            date.into_any_element(),
            chrome::tool(
                "insert-page-field",
                joyoffice_ui::IconName::Hash,
                self.t("insert.slide_number"),
                0x326797,
            )
            .disabled(disabled)
            .on_click(cx.listener(|s, _, w, cx| s.insert_text_field("slidenum", None, None, w, cx)))
            .into_any_element(),
        ]
    }

    fn can_insert_text_field(&self) -> bool {
        !self.loading
            && !self.composing()
            && self.benchmark.is_none()
            && self.format_targets().len() == 1
    }

    fn insert_text_field(
        &mut self,
        kind: &str,
        lang: Option<&str>,
        fixed: Option<&str>,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.can_insert_text_field() {
            return;
        }
        let Some(e) = &self.editor else { return };
        let value = if let Some(value) = fixed {
            Some(value.to_owned())
        } else if kind == "slidenum" {
            Some(e.session.model().slide_number(self.page).to_string())
        } else {
            DateTime::now_local().and_then(|now| field_text(kind, lang, now))
        };
        let Some(value) = value else {
            self.message = self.t("field.err_datetime").into();
            cx.notify();
            return;
        };
        if self.inline.is_none() {
            self.begin_selected_text(w, cx);
            if let Some(inline) = &mut self.inline {
                let end = inline.draft.body.text().len();
                inline.draft.move_to(end, false);
            }
        }
        let Some(inline) = &mut self.inline else {
            return;
        };
        let result = if fixed.is_some() {
            inline.draft.replace(None, &value, false, None)
        } else {
            inline.draft.insert_field(kind, &value, lang)
        };
        match result {
            Ok(()) => {
                self.publish_inline(w, cx);
                self.focus.focus(w, cx);
                w.notify_text_input_changed(&self.focus);
            }
            Err(error) => {
                self.message = error;
                cx.notify();
            }
        }
    }
}
