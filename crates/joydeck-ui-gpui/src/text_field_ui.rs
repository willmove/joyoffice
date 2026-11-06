//! Insert a native field or fixed date using the ordinary canvas text transaction.
use super::*;
use gpui_kit::component::{Sizable, button::ButtonVariants, popover::Popover};
use joydeck_text::datetime::{DATE_KINDS, DateTime, field_text};

const DATE_LABELS: [&str; 13] = [
    "短日期",
    "星期和日期",
    "日、月、年",
    "月、日、年",
    "日-月-短年",
    "月和年份",
    "短月和年份",
    "日期和时间",
    "日期和时间（含秒）",
    "24 小时时间",
    "24 小时时间（含秒）",
    "12 小时时间",
    "12 小时时间（含秒）",
];

fn date_language(index: usize) -> &'static str {
    if index == 0 { "zh-CN" } else { "en-US" }
}

impl Viewer {
    #[inline(never)]
    pub(super) fn text_field_group(&self, cx: &mut Context<Self>) -> Div {
        let disabled = !self.can_insert_text_field();
        let view = cx.entity().downgrade();
        let date = Popover::new("insert-date-menu")
            .trigger(
                Button::new("insert-date")
                    .ghost()
                    .xsmall()
                    .h(px(25.))
                    .label("日期和时间…")
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
                for (i, (name, kind)) in DATE_LABELS.into_iter().zip(DATE_KINDS).enumerate() {
                    let lang = date_language(i);
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
                                .label("固定")
                                .accessibility_label(format!("固定{name}"))
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
        chrome::group(
            "字段",
            div()
                .flex()
                .flex_col()
                .gap_1()
                .child(
                    Button::new("insert-page-field")
                        .ghost()
                        .xsmall()
                        .h(px(25.))
                        .label("幻灯片编号")
                        .disabled(disabled)
                        .on_click(cx.listener(|s, _, w, cx| {
                            s.insert_text_field("slidenum", None, None, w, cx)
                        })),
                )
                .child(date),
        )
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
            self.message = "无法获取此日期或时间".into();
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
