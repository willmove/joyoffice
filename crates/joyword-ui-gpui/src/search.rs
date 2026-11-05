//! Search and replacement controls over the document's existing engine session.
use super::*;
use gpui::component::{
    WindowExt,
    checkbox::Checkbox,
    input::{Input, InputState},
};
use gpui::{EntityInputHandler, Focusable};
use serde_json::json;

impl JoyWord {
    pub(super) fn search_ribbon(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let tr = |zh, en| self.shell.tr(zh, en);
        div()
            .flex()
            .flex_col()
            .gap_1()
            .child(
                Button::new("find-document")
                    .ghost()
                    .xsmall()
                    .label(tr("查找…", "Find…"))
                    .disabled(!self.format_ready("edit.findNext"))
                    .on_click(cx.listener(|v, _, w, cx| v.open_find(false, w, cx))),
            )
            .child(
                Button::new("replace-document")
                    .ghost()
                    .xsmall()
                    .label(tr("替换…", "Replace…"))
                    .disabled(!self.format_ready("edit.findNext"))
                    .on_click(cx.listener(|v, _, w, cx| v.open_find(true, w, cx))),
            )
            .child(
                Button::new("goto-page")
                    .ghost()
                    .xsmall()
                    .label(tr("转到页…", "Go to page…"))
                    .disabled(!self.format_ready("edit.goto"))
                    .on_click(cx.listener(|v, _, w, cx| v.open_go_to(w, cx))),
            )
    }

    pub(super) fn open_go_to(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        self.fields_form(
            self.shell.tr("转到页", "Go to page"),
            "edit.goto",
            vec![(self.shell.tr("页码", "Page number"), String::new())],
            false,
            |values| {
                let page = values
                    .first()
                    .and_then(|value| value.trim().parse::<u64>().ok())
                    .filter(|page| *page > 0)
                    .ok_or("请输入从 1 开始的页码")?;
                Ok(json!({"page": page}))
            },
            w,
            cx,
        );
    }

    pub(super) fn open_find(&mut self, replace: bool, w: &mut Window, cx: &mut Context<Self>) {
        if !self.format_ready("edit.findNext") {
            return;
        }
        let Some(reader) = self.canvas.reader.as_ref() else {
            return;
        };
        let document = reader.engine().document_id();
        let tr = |zh, en| self.shell.tr(zh, en);
        let title = if replace {
            tr("查找与替换", "Find and replace")
        } else {
            tr("查找", "Find")
        };
        let find_label = tr("查找内容", "Find what");
        let replacement_label = tr("替换为", "Replace with");
        let options = [
            ("edit.findPrevious", tr("上一处", "Previous")),
            ("edit.findNext", tr("下一处", "Next")),
            ("edit.replace", tr("替换并查找", "Replace and find")),
            ("edit.replaceAll", tr("全部替换", "Replace all")),
        ];
        let search = cx.new(|cx| {
            let mut input = InputState::new(w, cx).placeholder(find_label);
            input.set_value(reader.engine().find.query.clone(), w, cx);
            input
        });
        let replacement = cx.new(|cx| {
            let mut input = InputState::new(w, cx).placeholder(replacement_label);
            input.set_value(reader.engine().find.replace.clone(), w, cx);
            input
        });
        let choices = [
            (
                "matchCase",
                tr("区分大小写", "Match case"),
                reader.engine().find.match_case,
            ),
            (
                "wholeWord",
                tr("全字匹配", "Whole words"),
                reader.engine().find.whole_word,
            ),
            (
                "regex",
                tr("正则表达式", "Regular expression"),
                reader.engine().find.regex,
            ),
        ];
        let checked = cx.new(|_| choices.map(|(_, _, enabled)| enabled));
        let message = cx.new(|_| String::new());
        let owner = cx.weak_entity();
        let input_focus = search.read(cx).focus_handle(cx);
        w.open_dialog(cx, move |dialog, _, cx| {
            let mut body = div().flex().flex_col().gap_2()
                .child(Input::new(&search).aria_label(find_label));
            if replace {
                body = body.child(Input::new(&replacement).aria_label(replacement_label));
            }
            body = body.child(div().flex().gap_3().flex_wrap().children(
                choices.into_iter().enumerate().map(|(index, (id, label, _))| {
                    let selected = checked.read(cx)[index];
                    let checked = checked.clone();
                    Checkbox::new(id).label(label).checked(selected).on_change(move |enabled, _, cx| {
                        checked.update(cx, |checked, cx| {
                            checked[index] = *enabled;
                            cx.notify();
                        });
                    })
                }),
            ));
            let mut actions = div().flex().gap_2().flex_wrap();
            for (command, label) in options.into_iter().take(if replace { 4 } else { 2 }) {
                let (owner, search, replacement, message) =
                    (owner.clone(), search.clone(), replacement.clone(), message.clone());
                let checked = checked.clone();
                let button = Button::new(command).label(label).on_click(move |_, w, cx| {
                    if [&search, &replacement].iter().any(|input| {
                        input.update(cx, |input, cx| input.marked_text_range(w, cx).is_some())
                    }) {
                        return;
                    }
                    let query = search.read(cx).value().to_string();
                    let with = replacement.read(cx).value().to_string();
                    let [match_case, whole_word, regex] = *checked.read(cx);
                    let result = owner.update(cx, |v, cx| {
                        if v.canvas.reader.as_ref().is_none_or(|r| r.engine().document_id() != document) {
                            return Err("文档已变化，请重新打开查找".into());
                        }
                        if query.is_empty() {
                            return Err("请输入查找内容".into());
                        }
                        if query.len() > 4096 || with.len() > 4096 {
                            return Err("查找与替换各最多 4096 字节".into());
                        }
                        let args = json!({
                            "text": query, "with": with, "special": false,
                            "matchCase": match_case, "wholeWord": whole_word, "regex": regex, "wildcards": false,
                            "matchPrefix": false, "matchSuffix": false, "ignorePunct": false, "ignoreSpace": false,
                        });
                        v.try_edit(command, &args, w, cx)
                    });
                    message.update(cx, |message, cx| {
                        *message = match result {
                            Ok(Ok(value)) if command == "edit.replaceAll" => format!("已替换 {} 处", value["replaced"]),
                            Ok(Ok(value)) if command == "edit.replace" => format!("还有 {} 处匹配", value["remaining"]),
                            Ok(Ok(value)) => format!("{} / {}", value["index"].as_u64().unwrap_or(0) + 1, value["count"]),
                            Ok(Err(error)) => error,
                            Err(error) => error.to_string(),
                        };
                        cx.notify();
                    });
                    search.read(cx).focus_handle(cx).focus(w, cx);
                });
                actions = actions.child(div().debug_selector(move || command.into()).child(button));
            }
            let close = owner.clone();
            dialog.title(title).width(px(480.))
                .child(body.child(actions).child(div().child(message.read(cx).clone())))
                .on_close(move |_, w, cx| {
                    let _ = close.update(cx, |v, cx| w.focus(&v.focus, cx));
                })
        });
        input_focus.focus(w, cx);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::{Modifiers, TestAppContext};
    use std::sync::atomic::AtomicBool;

    #[gpui::test]
    fn search_shortcuts_and_replace_all_use_the_document_session(cx: &mut TestAppContext) {
        cx.update(|cx| {
            gpui_kit::init(cx);
            bind_document_keys(cx);
        });
        let mut editor = None;
        let (_, cx) = cx.add_window_view(|w, cx| {
            let view = cx.new(|cx| JoyWord::new(w, cx));
            editor = Some(view.clone());
            gpui::base::Root::new(view, w, cx)
        });
        let view = editor.unwrap();
        cx.update(|w, cx| {
            view.update(cx, |v, cx| {
                let mut reader =
                    joyword_shell::reader::Reader::blank(&AtomicBool::new(false)).unwrap();
                reader
                    .execute("text.insert", &json!({"text":"中"}))
                    .unwrap();
                reader.execute("format.bold", &json!({})).unwrap();
                reader
                    .execute("text.insert", &json!({"text":"文e\u{301} 中文e\u{301}"}))
                    .unwrap();
                reader
                    .execute("edit.find", &json!({"text":"中文e\u{301}","with":"新"}))
                    .unwrap();
                let id = reader.engine().document_id();
                v.canvas.reader = Some(reader);
                v.install_input(id, cx);
                w.focus(&v.focus, cx);
            });
            w.draw(cx).clear(cx);
        });
        for shortcut in ["ctrl-f", "f5"] {
            cx.simulate_keystrokes(shortcut);
            cx.update(|w, cx| {
                assert!(w.has_active_dialog(cx));
                w.close_dialog(cx);
                w.draw(cx).clear(cx);
            });
        }
        cx.simulate_keystrokes("ctrl-h");
        cx.update(|w, cx| w.draw(cx).clear(cx));
        let all = cx.debug_bounds("edit.replaceAll").unwrap();
        cx.simulate_click(all.center(), Modifiers::default());
        view.read_with(cx, |v, _| {
            let r = v.canvas.reader.as_ref().unwrap();
            assert_eq!(
                r.engine().doc.plain_text(r.engine().sel.focus.story),
                "新 新"
            );
        });
        cx.update(|w, cx| {
            w.close_dialog(cx);
            view.update(cx, |v, cx| {
                v.try_edit("edit.undo", &json!({}), w, cx).unwrap();
                let r = v.canvas.reader.as_ref().unwrap();
                assert_eq!(
                    r.engine().doc.plain_text(r.engine().sel.focus.story),
                    "中文e\u{301} 中文e\u{301}"
                );
            });
        });
    }
}
