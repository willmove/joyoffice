//! Insert a native field or fixed date using the ordinary canvas text transaction.
use super::*;
use gpui_kit::component::{Sizable, button::ButtonVariants, popover::Popover};
use joydeck_engine::{Command, existing_footer_summary};
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

#[derive(Clone, Copy)]
enum FooterScope {
    CurrentMissing,
    AllMissing,
    Existing,
    ClearExisting,
}

impl FooterScope {
    fn label(self) -> &'static str {
        match self {
            Self::CurrentMissing => "为当前缺失页脚的幻灯片添加",
            Self::AllMissing => "为所有缺失页脚的幻灯片添加",
            Self::Existing => "更新全稿已有页脚",
            Self::ClearExisting => "清空全稿已有页脚文字",
        }
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
    pub(super) fn footer_group(&self, cx: &mut Context<Self>) -> Div {
        let view = cx.entity().downgrade();
        let disabled = self.editor.is_none() || self.loading || self.composing();
        chrome::group(
            "页脚",
            Popover::new("footer-menu")
                .trigger(
                    Button::new("insert-footer")
                        .ghost()
                        .small()
                        .label("页脚…")
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
                                .label(scope.label())
                                .on_click(move |_, w, cx| {
                                    let _ = popup.update(cx, |p, cx| p.dismiss(w, cx));
                                    let _ = view.update(cx, |s, cx| s.footer_dialog(scope, w, cx));
                                })
                        }),
                    )
                }),
        )
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
                            Some(text) => ("页脚文字", text),
                            None => ("现有页脚文字不同，请输入统一文字", String::new()),
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
            vec![("页脚文字", String::new())]
        };
        self.fields_dialog(
            if matches!(scope, FooterScope::ClearExisting) {
                "清空全稿已有页脚文字？保留页脚占位框。"
            } else {
                scope.label()
            },
            fields,
            w,
            cx,
            move |s, values, w, cx| {
                if matches!(scope, FooterScope::CurrentMissing)
                    && s.editor.as_ref().map(|e| e.session.selection().page) != Some(page)
                {
                    return Err("当前幻灯片已变化，请重新打开此对话框".into());
                }
                let text = values.into_iter().next().unwrap_or_default();
                if !matches!(scope, FooterScope::ClearExisting) && text.trim().is_empty() {
                    return Err("请输入页脚文字".into());
                }
                if !s.try_dispatch(scope.command(text), w, cx)? {
                    return Err("没有需要更改的页脚".into());
                }
                Ok(())
            },
        );
    }

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
