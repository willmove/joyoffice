use super::*;
use gpui_kit::component::Sizable;
use joydeck::{compare, preferences};
impl Editor {
    pub(super) fn remember_path(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        cx.spawn(async move |view, cx| {
            let result = cx
                .background_executor()
                .spawn(async move { preferences::remember(&preferences::root(), &path) })
                .await;
            let _ = view.update(cx, |s, cx| {
                match result {
                    Ok(p) => {
                        if p.generation >= s.preferences.generation {
                            s.preferences = p;
                        }
                    }
                    Err(_) => s.message.push_str("；最近文件记录未保存"),
                };
                cx.notify();
            });
        })
        .detach();
    }
    pub(super) fn save_preferences(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        let (zoom, inspector, notes) = (self.zoom, self.inspector_open, self.show_notes);
        let bounds = w.window_bounds().get_bounds();
        let width = (f32::from(bounds.size.width).round() as u32).clamp(800, 2560);
        let height = (f32::from(bounds.size.height).round() as u32).clamp(600, 1600);
        cx.spawn(async move |view, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    preferences::update(&preferences::root(), |p| {
                        p.zoom = zoom;
                        p.inspector = inspector;
                        p.notes = notes;
                        p.width = width;
                        p.height = height;
                    })
                })
                .await;
            let _ = view.update(cx, |s, cx| {
                match result {
                    Ok(p) => {
                        if p.generation >= s.preferences.generation {
                            s.preferences = p;
                        }
                        s.message = "布局偏好已保存；下次启动恢复".into();
                    }
                    Err(e) => s.message = e,
                };
                cx.notify();
            });
        })
        .detach();
    }
    pub(super) fn compare_file(&mut self, cx: &mut Context<Self>) {
        self.finish_text(cx);
        let path = PathBuf::from(self.comparison_path.read(cx).value().trim());
        let base = self.session.deck.clone();
        let id = self.session.id;
        let revision = self.session.revision;
        self.message = "正在比较文稿…".into();
        cx.notify();
        cx.spawn(async move |view, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    let other = joydeck::document::open(&path, &AtomicBool::new(false))?;
                    Ok::<_, String>((compare::compare(&base, &other)?, Arc::new(other)))
                })
                .await;
            let _ = view.update(cx, |s, cx| {
                s.session.sync();
                if s.session.id != id || s.session.revision != revision {
                    s.message = "文稿已改变，请重新比较".into();
                } else {
                    match result {
                        Ok((report, other)) => {
                            s.message = format!(
                                "比较完成：{}项{}",
                                report.differences.len(),
                                if report.truncated {
                                    "（已达到4096项上限）"
                                } else {
                                    ""
                                }
                            );
                            s.comparison = Some((report, other));
                            s.comparison_pick = 0;
                        }
                        Err(e) => s.message = format!("比较失败：{e}"),
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }
    pub(super) fn accept_comparison(&mut self, cx: &mut Context<Self>) {
        self.session.sync();
        let command = self
            .comparison
            .as_ref()
            .ok_or("请先比较文稿".to_owned())
            .and_then(|(report, reference)| {
                compare::accept_object(&self.session.deck, reference, report, self.comparison_pick)
            });
        match command {
            Ok(command) => self.command(command, cx),
            Err(e) => {
                self.message = e;
                cx.notify();
            }
        }
    }
    pub(super) fn maintenance_pane(&self, cx: &mut Context<Self>) -> AnyElement {
        let button = |id: &'static str, label: &'static str| Button::new(id).label(label).small();
        let mut pane = div()
            .id("maintenance-pane")
            .size_full()
            .p_4()
            .flex()
            .flex_col()
            .gap_3()
            .overflow_y_scroll()
            .bg(rgb(0xffffff))
            .text_size(px(13.));
        if self.ribbon == "比较" {
            pane=pane.child("将当前文稿与另一文稿比较")
                .child(Input::new(&self.comparison_path).small())
                .child(button("compare-file","比较").on_click(cx.listener(|s,_,_,cx|s.compare_file(cx))))
                .child("可逐个接受对象内容更改，并使用撤销恢复。目标须已有所需图片/版式资源；页面、主题和审阅差异仅报告。")
                .child(button("accept-difference","接受选中的对象更改").on_click(cx.listener(|s,_,_,cx|s.accept_comparison(cx))));
            if let Some((report, _)) = &self.comparison {
                for (i, row) in report.differences.iter().enumerate() {
                    let kind = match row.kind.as_str() {
                        "object.changed" => "对象修改",
                        "object.added" => "新增对象",
                        "object.deleted" => "删除对象",
                        "object.order" => "对象顺序",
                        "page.order" => "页面顺序",
                        "page.added" => "新增页",
                        "page.deleted" => "删除页",
                        "page" => "页面属性",
                        _ => "文稿设置",
                    };
                    let page = row
                        .slide
                        .as_ref()
                        .and_then(|id| self.session.deck.slides.iter().position(|s| &s.id == id))
                        .map(|p| format!("第{}页", p + 1))
                        .unwrap_or_default();
                    pane = pane.child(
                        Button::new(("diff-row", i))
                            .small()
                            .label(format!(
                                "{} {page} {kind} {}",
                                if i == self.comparison_pick {
                                    "●"
                                } else {
                                    "○"
                                },
                                row.fields
                                    .iter()
                                    .map(|field| match field.as_str() {
                                        "text" => "文字",
                                        "x" | "y" => "位置",
                                        "width" | "height" => "尺寸",
                                        "fill" => "颜色",
                                        "rotation" => "旋转",
                                        "opacity" => "透明度",
                                        "runs" => "文字格式",
                                        "chart" => "图表数据",
                                        "table" => "表格内容",
                                        "image" => "图片",
                                        "fontSize" => "字号",
                                        _ => "其它属性",
                                    })
                                    .collect::<Vec<_>>()
                                    .join("、")
                            ))
                            .on_click(cx.listener(move |s, _, _, cx| {
                                s.comparison_pick = i;
                                cx.notify();
                            })),
                    );
                }
                if report.differences.is_empty() {
                    pane = pane.child("文稿内容相同。源PPTX容器及诊断警告不参与比较。");
                }
            }
        } else {
            pane = pane
                .child(format!(
                    "JoyDeck {} · {} / {} 开发预览",
                    env!("CARGO_PKG_VERSION"),
                    std::env::consts::OS,
                    std::env::consts::ARCH
                ))
                .child(format!(
                    "当前布局：缩放{:.0}% · 格式栏{} · 备注{}",
                    self.zoom * 100.,
                    if self.inspector_open {
                        "打开"
                    } else {
                        "收起"
                    },
                    if self.show_notes { "显示" } else { "隐藏" }
                ))
                .child(
                    button("prefs-save", "保存当前窗口与布局偏好")
                        .on_click(cx.listener(|s, _, w, cx| s.save_preferences(w, cx))),
                )
                .child(
                    button("copy-diagnostics", "复制诊断摘要").on_click(cx.listener(
                        |s, _, _, cx| {
                            cx.write_to_clipboard(ClipboardItem::new_string(
                                joydeck::diagnostics::report().to_string(),
                            ));
                            s.message = "诊断摘要已复制（版本/平台/能力）".into();
                            cx.notify();
                        },
                    )),
                )
                .child(
                    button("prefs-clear-recent", "清空最近文件").on_click(cx.listener(
                        |_, _, _, cx| {
                            cx.spawn(async move |view, cx| {
                                let result = cx
                                    .background_executor()
                                    .spawn(async {
                                        preferences::update(&preferences::root(), |p| {
                                            p.recent.clear()
                                        })
                                    })
                                    .await;
                                let _ = view.update(cx, |s, cx| {
                                    match result {
                                        Ok(p) => {
                                            if p.generation >= s.preferences.generation {
                                                s.preferences = p;
                                            }
                                            s.message = "最近文件记录已清空".into();
                                        }
                                        Err(e) => s.message = e,
                                    };
                                    cx.notify();
                                });
                            })
                            .detach();
                        },
                    )),
                )
                .child("最近文件（每份文稿单独打开；当前修改保留）");
            for (i, path) in self.preferences.recent.iter().enumerate() {
                let path = path.clone();
                pane = pane.child(
                    Button::new(("recent-file", i))
                        .small()
                        .label(
                            path.file_name()
                                .unwrap_or_default()
                                .to_string_lossy()
                                .to_string(),
                        )
                        .on_click(cx.listener(move |s, _, _, cx| {
                            if let Err(e) = open_path(path.clone(), cx) {
                                s.message = e.to_string();
                                cx.notify();
                            }
                        })),
                );
            }
            pane=pane.child("使用与恢复")
                .child("⌘/Ctrl+S 保存；⌘/Ctrl+Z 撤销。恢复副本从“文件 → 恢复”打开；另存到新文件后再检查原件。")
                .child("输出到新路径。PPTX公开输出有能力损失提示；静态页MP4不含音轨、动画、转场或录制。")
                .child("开发包的README、BUILD-INFO、SBOM与SHA256清单记录支持范围和依赖；正式签名和跨平台验收以状态文档为准。");
        }
        pane.into_any_element()
    }
}
