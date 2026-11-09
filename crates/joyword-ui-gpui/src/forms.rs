//! JoyOffice's version-bound native field forms, shared by authoring tools.
use super::*;
use gpui::component::{
    Sizable, WindowExt,
    input::{Input, InputState},
};
use gpui::{EntityInputHandler, Focusable};
use serde_json::Value;
use std::rc::Rc;
pub(super) fn show(v: &Value) -> String {
    if let Some(s) = v.as_str() {
        s.into()
    } else if let Some(n) = v.as_f64() {
        format!("{n:.3}")
            .trim_end_matches('0')
            .trim_end_matches('.')
            .into()
    } else {
        String::new()
    }
}
impl JoyWord {
    pub(super) fn try_edit(
        &mut self,
        id: &str,
        args: &Value,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<Value, String> {
        if !self.format_ready(id) {
            return Err(self
                .canvas
                .reader
                .as_ref()
                .and_then(|r| r.formatting_disabled(id))
                .unwrap_or("编辑暂不可用")
                .into());
        }
        let mut result = Err("文档当前不可编辑".into());
        self.apply_input(w, cx, |r| {
            result = r.execute(id, args);
            result.as_ref().map(|_| ()).map_err(Clone::clone)
        });
        if result.is_ok() {
            self.advanced.key = None;
            w.focus(&self.focus, cx);
        }
        result
    }
    #[allow(clippy::too_many_arguments)]
    pub(super) fn fields_form(
        &mut self,
        title: &'static str,
        command: &'static str,
        fields: Vec<(&'static str, String)>,
        changed_only: bool,
        parse: impl Fn(&[String]) -> Result<Value, String> + 'static,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.fields_action(
            title,
            command,
            fields,
            changed_only,
            parse,
            move |v, args, w, cx| v.try_edit(command, args, w, cx),
            w,
            cx,
        );
    }
    #[allow(clippy::too_many_arguments)]
    pub(super) fn fields_action(
        &mut self,
        title: &'static str,
        command: &'static str,
        fields: Vec<(&'static str, String)>,
        changed_only: bool,
        parse: impl Fn(&[String]) -> Result<Value, String> + 'static,
        apply: impl Fn(&mut Self, &Value, &mut Window, &mut Context<Self>) -> Result<Value, String>
        + 'static,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.format_ready(command) {
            return;
        }
        let Some(r) = &self.canvas.reader else {
            return;
        };
        let target = (
            r.engine().document_id(),
            r.engine().rev(),
            r.presentation_revision(),
            r.engine().sel.clone(),
        );
        let original: Vec<_> = fields.iter().map(|(_, v)| v.clone()).collect();
        let inputs: Vec<_> = fields
            .into_iter()
            .map(|(label, value)| {
                let state = cx.new(|cx| {
                    let mut s = InputState::new(w, cx).placeholder("留空保持原值");
                    s.set_value(value, w, cx);
                    s
                });
                (label, state)
            })
            .collect();
        let view = cx.entity().downgrade();
        let error = cx.new(|_| String::new());
        let parse = Rc::new(parse);
        let apply = Rc::new(apply);
        w.open_dialog(cx, move |d, _, cx| {
            let mut body = div()
                .id("authoring-form-fields")
                .flex()
                .flex_col()
                .gap_2()
                .max_h(px(480.))
                .overflow_y_scroll();
            for (label, input) in &inputs {
                body = body
                    .child(div().text_sm().child(*label))
                    .child(Input::new(input).aria_label(*label).xsmall());
            }
            let (view, close, cancel) = (view.clone(), view.clone(), view.clone());
            let inputs = inputs.clone();
            let original = original.clone();
            let target = target.clone();
            let error2 = error.clone();
            let parse = parse.clone();
            let apply = apply.clone();
            d.title(title)
                .width(px(470.))
                .child(body)
                .child(
                    div()
                        .text_color(rgb(0xc42b1c))
                        .child(error.read(cx).clone()),
                )
                .on_close(move |_, w, cx| {
                    let _ = close.update(cx, |v, cx| w.focus(&v.focus, cx));
                })
                .child(
                    div()
                        .flex()
                        .justify_end()
                        .gap_2()
                        .debug_selector(|| "authoring-actions".into())
                        .child(
                            Button::new("cancel-authoring")
                                .ghost()
                                .label("取消")
                                .on_click(move |_, w, cx| {
                                    w.close_dialog(cx);
                                    let _ = cancel.update(cx, |v, cx| w.focus(&v.focus, cx));
                                }),
                        )
                        .child(
                            Button::new("apply-authoring")
                                .primary()
                                .label("应用")
                                .on_click(move |_, w, cx| {
                                    let _ = view.update(cx, |v, cx| {
                                        if v.canvas.reader.as_ref().map(|r| {
                                            (
                                                r.engine().document_id(),
                                                r.engine().rev(),
                                                r.presentation_revision(),
                                                r.engine().sel.clone(),
                                            )
                                        }) != Some(target.clone())
                                        {
                                            error2.update(cx, |e, cx| {
                                                *e = "文档或选区已变化，请重新打开。".into();
                                                cx.notify();
                                            });
                                            return;
                                        }
                                        if inputs.iter().any(|(_, s)| {
                                            s.update(cx, |s, cx| {
                                                s.marked_text_range(w, cx).is_some()
                                            })
                                        }) {
                                            return;
                                        }
                                        let values: Vec<String> = inputs
                                            .iter()
                                            .enumerate()
                                            .map(|(i, (_, s))| {
                                                let val = s.read(cx).value().to_string();
                                                if changed_only && val.trim() == original[i].trim()
                                                {
                                                    String::new()
                                                } else {
                                                    val
                                                }
                                            })
                                            .collect();
                                        let result =
                                            parse(&values).and_then(|args| apply(v, &args, w, cx));
                                        match result {
                                            Ok(result) => {
                                                if command == "styles.create" {
                                                    v.style_pick =
                                                        result["id"].as_str().map(str::to_string);
                                                    v.styles_open = true;
                                                }
                                                w.close_dialog(cx);
                                                w.focus(&v.focus, cx);
                                            }
                                            Err(e) => {
                                                error2.update(cx, |v, cx| {
                                                    *v = e;
                                                    cx.notify();
                                                });
                                                if let Some((_, first)) = inputs.first() {
                                                    w.focus(&first.focus_handle(cx), cx);
                                                }
                                            }
                                        }
                                    });
                                }),
                        ),
                )
        });
    }
}
