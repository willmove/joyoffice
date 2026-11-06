use super::*;
use gpui::component::WindowExt;
use gpui::{Entity, RenderImage, img};
use joyword_shell::printing::PrintJob;
use serde_json::{Value, json};
use std::sync::Arc;

struct Preview {
    job: PrintJob,
    index: usize,
    loading: bool,
    image: Option<(Arc<RenderImage>, f32, f32)>,
    error: Option<String>,
}

fn preview_page(state: &Entity<Preview>, index: usize, cx: &mut App) {
    state.update(cx, |preview, cx| {
        if preview.loading || index >= preview.job.page_count() {
            return;
        }
        preview.index = index;
        preview.loading = true;
        preview.error = None;
        if let Some((image, _, _)) = preview.image.take() {
            cx.drop_image(image, None);
        }
        let job = preview.job.clone();
        cx.spawn(async move |state, cx| {
            let rendered = cx
                .background_executor()
                .spawn(async move {
                    joyword_shell::reader::guarded(|| {
                        let bytes = job.preview_png(index)?;
                        let mut image = image::load_from_memory(&bytes)
                            .map_err(|error| error.to_string())?
                            .into_rgba8();
                        let (width, height) = (image.width() as f32, image.height() as f32);
                        let scale = (500. / width).min(460. / height);
                        for pixel in image.pixels_mut() {
                            pixel.0.swap(0, 2);
                        }
                        Ok((
                            Arc::new(RenderImage::new(vec![image::Frame::new(image)])),
                            width * scale,
                            height * scale,
                        ))
                    })
                })
                .await;
            let _ = state.update(cx, |preview, cx| {
                preview.loading = false;
                match rendered {
                    Ok(image) => preview.image = Some(image),
                    Err(error) => preview.error = Some(error),
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    });
}

impl JoyWord {
    pub(super) fn open_print_preview(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        let Some(reader) = self.canvas.reader.as_ref() else {
            return;
        };
        let (job, index) = match reader.print_preview_job() {
            Ok(job) => job,
            Err(error) => {
                self.shell.notice = Some(error);
                cx.notify();
                return;
            }
        };
        self.file_page = false;
        let state = cx.new(|_| Preview {
            job,
            index,
            loading: false,
            image: None,
            error: None,
        });
        preview_page(&state, index, cx);
        let focus = self.focus.clone();
        let title = self.shell.tr("打印预览", "Print preview");
        let loading = self.shell.tr("正在生成页面…", "Rendering page…");
        let previous = self.shell.tr("上一页", "Previous page");
        let next = self.shell.tr("下一页", "Next page");
        w.open_dialog(cx, move |dialog, _, cx| {
            let view = state.read(cx);
            let (index, count) = (view.index, view.job.page_count());
            let (back, forward, close) = (state.clone(), state.clone(), state.clone());
            let focus = focus.clone();
            let mut body = div().flex().flex_col().items_center().gap_2().child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        Button::new("print-preview-previous")
                            .label(previous)
                            .disabled(view.loading || index == 0)
                            .on_click(move |_, _, cx| {
                                preview_page(&back, index.saturating_sub(1), cx)
                            }),
                    )
                    .child(format!("{} / {count}", index + 1))
                    .child(
                        Button::new("print-preview-next")
                            .label(next)
                            .disabled(view.loading || index + 1 >= count)
                            .on_click(move |_, _, cx| preview_page(&forward, index + 1, cx)),
                    ),
            );
            if let Some((image, width, height)) = &view.image {
                body = body.child(img(image.clone()).w(px(*width)).h(px(*height)));
            } else if view.loading {
                body = body.child(loading);
            } else if let Some(error) = &view.error {
                body = body.child(error.clone());
            }
            dialog
                .title(title)
                .width(px(550.))
                .child(body)
                .on_close(move |_, w, cx| {
                    close.update(cx, |preview, cx| {
                        if let Some((image, _, _)) = preview.image.take() {
                            cx.drop_image(image, None);
                        }
                    });
                    w.focus(&focus, cx);
                })
        });
        cx.notify();
    }

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
