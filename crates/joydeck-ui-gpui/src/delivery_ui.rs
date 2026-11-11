//! Snapshot delivery controls. All encoding, printing and file publication run off the UI thread.
use super::*;
use joydeck_shell::output::{Options, PageLayout};
impl Viewer {
    pub(super) fn delivery_toolbar(&self, cx: &mut Context<Self>) -> Div {
        let mut row = div()
            .min_h(px(40.))
            .flex()
            .flex_wrap()
            .items_center()
            .gap_1()
            .px_2();
        for (label, kind, layout) in [
            ("PDF", "pdf", PageLayout::Slides),
            ("备注 PDF", "pdf", PageLayout::Notes),
            ("讲义 PDF", "pdf", PageLayout::Handouts { per_page: 3 }),
            ("PNG", "png", PageLayout::Slides),
            ("JPEG", "jpeg", PageLayout::Slides),
            ("动画 GIF", "gif", PageLayout::Slides),
            ("静态 MP4", "mp4", PageLayout::Slides),
            ("提纲", "outline", PageLayout::Slides),
            ("标题 VTT", "vtt", PageLayout::Slides),
            ("打印", "print", PageLayout::Slides),
        ] {
            row = row.child(
                Button::new(label)
                    .label(label)
                    .disabled(self.editor.is_none() || self.delivery_busy)
                    .on_click(cx.listener(move |s, _, w, cx| s.delivery_form(kind, layout, w, cx))),
            );
        }
        row.child(
            Button::new("cancel-output")
                .label("取消输出")
                .disabled(!self.delivery_busy)
                .on_click(cx.listener(|s, _, _, cx| {
                    s.delivery_cancel.cancel();
                    s.message = "正在取消输出…".into();
                    cx.notify();
                })),
        )
    }
    pub(super) fn delivery_form(
        &mut self,
        kind: &'static str,
        layout: PageLayout,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.editor.is_none() || self.delivery_busy {
            return;
        }
        self.end_inline(cx);
        let path = cx.new(|cx| {
            let mut s = InputState::new(w, cx);
            s.set_value("", w, cx);
            s.placeholder("新的输出文件或图片目录")
        });
        let pages = cx.new(|cx| {
            let mut s = InputState::new(w, cx);
            s.set_value("all", w, cx);
            s
        });
        let density = cx.new(|cx| {
            let mut s = InputState::new(w, cx);
            s.set_value("144", w, cx);
            s
        });
        let generation = self.generation;
        let error = cx.new(|_| String::new());
        let view = cx.entity().downgrade();
        let title = if kind == "print" {
            "打印幻灯片"
        } else {
            "导出文稿副本"
        };
        w.open_dialog(cx, move |dialog, _, cx| {
            let mut content = div()
                .id("output-fields")
                .max_h(px(340.))
                .overflow_y_scroll()
                .flex()
                .flex_col()
                .gap_2();
            if kind != "print" {
                content = content
                    .child(if ["png", "jpeg"].contains(&kind) {
                        "新的图片目录"
                    } else {
                        "新的输出文件"
                    })
                    .child(Input::new(&path));
            }
            content = content
                .child("页面：all 或 1,3,5（含隐藏页时请显式检查所选范围）")
                .child(Input::new(&pages))
                .child("PDF / 打印 DPI（36–300）；图片 72 DPI，GIF 每页 1 秒 / 10 帧")
                .child(Input::new(&density))
                .child(
                    div()
                        .text_color(rgb(0xc13d36))
                        .child(error.read(cx).clone()),
                );
            let (view, path, pages, density) =
                (view.clone(), path.clone(), pages.clone(), density.clone());
            let error = error.clone();
            dialog.title(title).child(content).child(
                Button::new("deliver")
                    .label(if kind == "print" {
                        "打开系统打印对话框"
                    } else {
                        "导出"
                    })
                    .on_click(move |_, w, cx| {
                        let _ = view.update(cx, |s, cx| {
                            if s.generation != generation {
                                s.message = "文稿已切换，请重新打开输出设置".into();
                                w.close_dialog(cx);
                                return;
                            }
                            for input in [&path, &pages, &density] {
                                if input.update(cx, |s, cx| s.marked_text_range(w, cx).is_some()) {
                                    s.message = "请先确认输入法组合文字".into();
                                    cx.notify();
                                    return;
                                }
                            }
                            let range = pages.read(cx).value().to_string();
                            let destination = path.read(cx).value().to_string();
                            let parsed = (|| {
                                let selected = if range.trim() == "all" {
                                    None
                                } else {
                                    Some(
                                        range
                                            .split(',')
                                            .map(|x| {
                                                x.trim()
                                                    .parse::<usize>()
                                                    .ok()
                                                    .and_then(|p| p.checked_sub(1))
                                                    .ok_or("页码从 1 开始")
                                            })
                                            .collect::<std::result::Result<Vec<_>, _>>()?,
                                    )
                                };
                                let dpi: f64 =
                                    density.read(cx).value().parse().map_err(|_| "DPI 无效")?;
                                if !dpi.is_finite() || !(36.0..=300.).contains(&dpi) {
                                    return Err("DPI 范围为 36–300");
                                }
                                if selected.as_ref().is_some_and(|pages: &Vec<usize>| {
                                    pages.is_empty()
                                        || pages.len() > 500
                                        || pages.iter().any(|p| {
                                            *p >= s
                                                .editor
                                                .as_ref()
                                                .map_or(0, |e| e.session.model().slides.len())
                                        })
                                }) {
                                    return Err("页面范围超出文稿或 500 页预算");
                                }
                                if kind != "print" && destination.trim().is_empty() {
                                    return Err("请输入新的输出路径");
                                }
                                Ok::<_, &str>(Options {
                                    format: kind.into(),
                                    pages: selected,
                                    dpi,
                                    layout,
                                    ..Default::default()
                                })
                            })();
                            match parsed {
                                Ok(options) => {
                                    w.close_dialog(cx);
                                    s.deliver(destination.into(), options, cx);
                                }
                                Err(e) => {
                                    s.message = e.into();
                                    error.update(cx, |value, _| *value = e.into());
                                    w.refresh();
                                    cx.notify();
                                }
                            }
                        });
                    }),
            )
        });
    }
    fn deliver(&mut self, path: PathBuf, options: Options, cx: &mut Context<Self>) {
        let Some(editor) = &self.editor else { return };
        if self.delivery_busy {
            return;
        }
        let model = editor.session.model_snapshot();
        let revision = editor.session.revision();
        let generation = self.generation;
        self.delivery_cancel = Default::default();
        let cancel = self.delivery_cancel.clone();
        self.delivery_busy = true;
        self.message = "正在输出文稿快照…".into();
        self.delivery_task = Some(cx.spawn(async move |view, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    let doc = joydeck_shell::Document::from_model(
                        "output snapshot".into(),
                        (*model).clone(),
                    )?;
                    if options.format == "print" {
                        joydeck_shell::output::print(&doc, &options, &cancel)
                            .map(|s| format!("打印：{s}"))
                    } else {
                        joydeck_shell::output::export(&doc, &path, &options, &cancel).map(|r| {
                            format!(
                                "输出完成：{} 页 / {} 个文件 / {} 字节",
                                r.pages.len(),
                                r.files,
                                r.bytes
                            )
                        })
                    }
                })
                .await;
            let _ = view.update(cx, |s, cx| {
                s.delivery_busy = false;
                if generation == s.generation {
                    s.message = result.unwrap_or_else(|e| format!("输出失败：{e}"));
                    eprintln!("R6 delivery revision={revision} {}", s.message);
                    cx.notify();
                }
            });
        }));
        cx.notify();
    }
}
