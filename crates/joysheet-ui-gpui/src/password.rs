use crate::desktop::{After, Desktop};
use gpui_kit::{
    component::{
        WindowExt,
        button::Button,
        input::{Input, InputState},
    },
    prelude::*,
    *,
};
use joysheet_engine::io::{PASSWORD_REQUIRED, WRONG_PASSWORD};
use serde_json::json;
use std::collections::VecDeque;

pub(crate) struct PasswordRequests(pub VecDeque<(String, bool)>);
impl Global for PasswordRequests {}

pub(crate) fn needs_password(path: &str, error: &str) -> bool {
    error == format!("'{path}' {PASSWORD_REQUIRED}")
}

impl Desktop {
    pub(crate) fn open_native_path(&mut self, path: String, password: Option<String>, cx: &mut Context<Self>) {
        let mut params = json!({"path": path});
        if let Some(password) = password {
            params["password"] = json!(password);
        }
        self.issue("file.open", params, After::NativeOpen(path), cx);
    }

    pub(crate) fn queue_password(&mut self, path: String, error: &str, cx: &mut Context<Self>) -> bool {
        let retry = error.starts_with(WRONG_PASSWORD);
        if !retry && !needs_password(&path, error) {
            return false;
        }
        cx.update_global::<PasswordRequests, _>(|requests, _| {
            if retry {
                requests.0.push_front((path, true));
            } else {
                requests.0.push_back((path, false));
            }
        });
        true
    }

    pub(crate) fn show_pending_password(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.window_id != self.snapshot.active_window || window.has_active_dialog(cx) {
            return;
        }
        let Some((path, retry)) = cx.update_global::<PasswordRequests, _>(|requests, _| requests.0.pop_front()) else {
            return;
        };
        let name = std::path::Path::new(&path).file_name().and_then(std::ffi::OsStr::to_str).unwrap_or(&path).to_owned();
        let title = self.label("输入工作簿密码", "Enter workbook password");
        let password_label = self.label("密码", "Password");
        let open = self.label("打开", "Open");
        let cancel = self.label("取消", "Cancel");
        let notice = self.label(
            "另存时会创建未加密副本，保存前不会写入自动恢复副本。",
            "Save As creates an unencrypted copy. Automatic recovery starts after you save that copy.",
        );
        let invalid = self.label("请输入不超过 1024 字节的密码。", "Enter a password up to 1024 bytes.");
        let error = cx.new(|_| if retry { self.label("密码不正确，请重试。", "Incorrect password. Try again.") } else { String::new() });
        let input = cx.new(|cx| InputState::new(window, cx).masked(true));
        let focus = input.focus_handle(cx);
        let view = cx.weak_entity();
        window.open_dialog(cx, move |dialog, _, cx| {
            let (path, input, error, view) = (path.clone(), input.clone(), error.clone(), view.clone());
            dialog
                .title(title.clone())
                .child(name.clone())
                .child(div().text_sm().child(notice.clone()))
                .child(Input::new(&input).aria_label(password_label.clone()))
                .child(div().text_sm().text_color(rgb(0xc42b1c)).child(error.read(cx).clone()))
                .child(
                    div()
                        .flex()
                        .gap_2()
                        .child(Button::new("cancel-workbook-password").label(cancel.clone()).on_click(|_, w, cx| w.close_dialog(cx)))
                        .child(Button::new("submit-workbook-password").label(open.clone()).on_click({
                            let invalid = invalid.clone();
                            move |_, w, cx| {
                                let password = input.read(cx).value().to_string();
                                if password.is_empty() || password.len() > 1024 {
                                    error.update(cx, |error, cx| {
                                        *error = invalid.clone();
                                        cx.notify();
                                    });
                                    return;
                                }
                                input.update(cx, |input, cx| input.set_value("", w, cx));
                                w.close_dialog(cx);
                                let _ = view.update(cx, |view, cx| view.open_native_path(path.clone(), Some(password), cx));
                            }
                        })),
                )
        });
        window.focus(&focus, cx);
    }
}
