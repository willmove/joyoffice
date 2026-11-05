use super::*;
use serde_json::{Value, json};

impl JoyWord {
    pub(super) fn open_print(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        if self.printing {
            return;
        }
        self.file_page = false;
        let tr = |zh, en| self.shell.tr(zh, en);
        self.fields_action(
            tr("打印", "Print"),
            "file.print",
            vec![(
                tr(
                    "页码范围（空白为全部，最多 32 页）",
                    "Pages (blank for all, up to 32)",
                ),
                String::new(),
            )],
            false,
            |values| Ok(json!({"pages":values.first()})),
            |v, args, _, cx| {
                let job = v
                    .canvas
                    .reader
                    .as_ref()
                    .ok_or("No document")?
                    .print_job(args["pages"].as_str().unwrap_or_default())?;
                v.printing = true;
                let cancelled = v.shell.tr("已取消打印", "Printing cancelled");
                let submitted = v.shell.tr(
                    "已提交到 Windows 打印队列",
                    "Submitted to the Windows print queue",
                );
                cx.spawn(async move |view, cx| {
                    let result = cx
                        .background_executor()
                        .spawn(async move { joyword_shell::reader::guarded(|| job.dialog()) })
                        .await;
                    let _ = view.update(cx, |v, cx| {
                        v.printing = false;
                        v.shell.notice = Some(match result {
                            Ok(true) => submitted.into(),
                            Ok(false) => cancelled.into(),
                            Err(error) => error,
                        });
                        cx.notify();
                    });
                })
                .detach();
                Ok(Value::Null)
            },
            w,
            cx,
        );
    }
}
