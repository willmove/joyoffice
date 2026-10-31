use super::*;
use joydeck::{export, interop, pptx_write};
impl Editor {
    pub(super) fn delivery(&mut self, kind: &str, w: &mut Window, cx: &mut Context<Self>) {
        self.finish_text(cx);
        self.cancel_gesture(&CancelGesture, w, cx);
        self.sync_document(cx);
        if kind == "show-first" || kind == "show-current" {
            self.prepare_show(w, cx);
            let start = if kind == "show-first" {
                self.session
                    .deck
                    .presentation
                    .order
                    .first()
                    .and_then(|id| self.session.deck.slides.iter().position(|s| &s.id == id))
                    .unwrap_or(0)
            } else {
                self.page
            };
            match super::super::slideshow::open(self.session.deck.clone(), start, cx) {
                Ok(_) => {}
                Err(e) => {
                    self.message = e.to_string();
                    cx.notify();
                }
            }
            return;
        }
        if self.delivery_busy {
            self.message = "已有导出任务；可先取消".into();
            cx.notify();
            return;
        }
        let path = PathBuf::from(self.delivery_fields[0].read(cx).value().trim());
        if path.as_os_str().is_empty() {
            self.message = "请填写导入文件、导出文件或新建图片文件夹的完整路径".into();
            cx.notify();
            return;
        }
        if kind == "pptx" {
            let losses = pptx_write::losses(&self.session.deck).join("\n");
            let weak = cx.entity().downgrade();
            w.open_dialog(cx, move |d, _, _| {
                let weak = weak.clone();
                d.title("新建PPTX的兼容性说明")
                    .child(losses.clone())
                    .child(
                        Button::new("confirm-lossy-pptx")
                            .label("接受这些损失，导出到新文件")
                            .on_click(move |_, w, cx| {
                                w.close_dialog(cx);
                                let _ = weak.update(cx, |s, cx| s.start_delivery("pptx", cx));
                            }),
                    )
                    .child(
                        Button::new("cancel-lossy")
                            .label("取消")
                            .on_click(|_, w, cx| w.close_dialog(cx)),
                    )
            });
            return;
        }
        if kind == "import" && self.session.dirty() {
            self.message = "导入会替换当前窗口文稿，请先保存当前修改或在新文稿窗口导入".into();
            cx.notify();
            return;
        }
        if kind == "import" {
            self.path.update(cx, |p, cx| p.set_value("", w, cx));
        }
        self.start_delivery(kind, cx);
    }
    fn start_delivery(&mut self, kind: &str, cx: &mut Context<Self>) {
        let path = PathBuf::from(self.delivery_fields[0].read(cx).value().trim());
        let range = self.delivery_fields[1].read(cx).value().to_string();
        let dpi = self.delivery_fields[2].read(cx).value().parse::<u32>();
        let kind = kind.to_owned();
        let deck = self.session.deck.clone();
        let id = self.session.id;
        let revision = self.session.revision;
        let cancel = Arc::new(AtomicBool::new(false));
        self.delivery_cancel = cancel.clone();
        self.delivery_busy = true;
        self.message = "正在处理不可变文稿快照…".into();
        cx.notify();
        cx.spawn(async move |view, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    (|| -> std::result::Result<Option<Deck>, String> {
                        match kind.as_str() {
                            "import" => {
                                let d = interop::import(&path)?;
                                joydeck::document::check_cancel(&cancel)?;
                                return Ok(Some(d));
                            }
                            "preserve" => {
                                if !path
                                    .extension()
                                    .is_some_and(|e| e.eq_ignore_ascii_case("pptx"))
                                {
                                    return Err("PPTX输出需要.pptx扩展名".into());
                                }
                                let source =
                                    deck.source.as_ref().ok_or("当前文稿没有原始PPTX源包")?;
                                export::save_parts(&path, &source.preserve(&deck)?, &cancel)?;
                            }
                            "pptx" => {
                                if !path
                                    .extension()
                                    .is_some_and(|e| e.eq_ignore_ascii_case("pptx"))
                                {
                                    return Err("PPTX输出需要.pptx扩展名".into());
                                }
                                export::save_parts(&path, &pptx_write::generate(&deck)?, &cancel)?;
                            }
                            _ => {
                                let options = export::Options {
                                    pages: export::pages(&deck, &range, false)?,
                                    dpi: dpi.map_err(|_| "DPI必须是整数")?,
                                    transparent: kind == "png-alpha",
                                    notes: kind == "pdf-notes",
                                };
                                if kind.starts_with("pdf") {
                                    if !path
                                        .extension()
                                        .is_some_and(|e| e.eq_ignore_ascii_case("pdf"))
                                    {
                                        return Err("PDF输出需要.pdf扩展名".into());
                                    }
                                    export::pdf(&path, &deck, &options, &cancel)?;
                                } else {
                                    export::images(
                                        &path,
                                        &deck,
                                        &options,
                                        if kind == "jpeg" {
                                            export::ImageFormat::Jpeg
                                        } else {
                                            export::ImageFormat::Png
                                        },
                                        &cancel,
                                    )?;
                                }
                            }
                        }
                        Ok(None)
                    })()
                })
                .await;
            let _ = view.update(cx, |s, cx| {
                s.delivery_busy = false;
                s.sync_document(cx);
                match result {
                    Ok(Some(d)) => {
                        if s.session.id != id || s.session.revision != revision {
                            s.message = "文稿已改变，已丢弃PPTX导入结果".into();
                        } else {
                            s.session = Session::from_deck(d, false);
                            s.renew_recovery();
                            s.page = 0;
                            s.selection.clear();
                            s.selected = None;
                            s.changed(cx);
                            s.message =
                                "已导入PPTX；请保存为原生文稿，或使用源包保留写回到新文件".into();
                        }
                    }
                    Ok(None) => {
                        s.message = "快照已导出到新目标；编辑器内容与dirty状态未改变".into()
                    }
                    Err(e) => s.message = format!("交付失败：{e}"),
                }
                cx.notify();
            });
        })
        .detach();
    }
    pub(super) fn delivery_controls(&self, cx: &mut Context<Self>) -> AnyElement {
        let mut panel = div()
            .flex()
            .flex_col()
            .gap_2()
            .child("文件交付（导出均新建，拒绝覆盖）");
        if !self.session.deck.warnings.is_empty() {
            panel = panel.child(format!(
                "兼容性诊断：{}项（当前显示为支持子集）",
                self.session.deck.warnings.len()
            ));
            for warning in self.session.deck.warnings.iter().take(8) {
                panel = panel.child(div().child(warning.message.clone()));
            }
        }
        for (i, label) in [
            "完整文件路径 / 新图片文件夹",
            "页范围：空=全部可见；例如1-3,5",
            "DPI：36–600（受像素预算限制）",
        ]
        .iter()
        .enumerate()
        {
            panel = panel.child(
                div()
                    .child(*label)
                    .child(Input::new(&self.delivery_fields[i])),
            );
        }
        for (kind, label) in [
            ("import", "导入PPTX / 原生文稿"),
            ("preserve", "保留源包写回PPTX（仅单行文字）"),
            ("pptx", "新建PPTX（需确认损失）"),
            ("png", "导出PNG文件夹"),
            ("png-alpha", "导出透明PNG文件夹"),
            ("jpeg", "导出JPEG文件夹"),
            ("pdf", "导出PDF（栅格+文本映射）"),
            ("pdf-notes", "导出PDF含备注"),
            ("show-first", "从首页全屏放映"),
            ("show-current", "从当前页全屏放映"),
        ] {
            panel = panel.child(
                Button::new(kind)
                    .label(label)
                    .on_click(cx.listener(move |s, _, w, cx| s.delivery(kind, w, cx))),
            );
        }
        panel
            .child(
                Button::new("cancel-delivery")
                    .label("取消导出/导入任务")
                    .on_click(cx.listener(|s, _, _, cx| {
                        s.delivery_cancel
                            .store(true, std::sync::atomic::Ordering::Relaxed);
                        s.message = "已请求取消；若输出已提交则保留已完成产物".into();
                        cx.notify();
                    })),
            )
            .into_any_element()
    }
}
