//! Recent files, original starter templates and independent document windows.
use super::*;
use gpui_kit::component::{Sizable, button::ButtonVariants};
use joydeck_engine::{Command, ContentAction, Session};

impl Viewer {
    pub(super) fn refresh_recent(&mut self, cx: &mut Context<Self>) {
        let root = joydeck_shell::editor::recovery_root();
        self.recent_task = Some(cx.spawn(async move |view, cx| {
            let result = cx
                .background_executor()
                .spawn(async move { joydeck_shell::storage::recent_files(&root) })
                .await;
            let _ = view.update(cx, |s, cx| {
                s.recent_paths = result;
                cx.notify();
            });
        }));
    }
    pub(super) fn recent_panel(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let mut panel = div()
            .id("recent-files")
            .flex()
            .flex_col()
            .gap_2()
            .max_h(px(190.))
            .overflow_y_scroll()
            .child(div().text_lg().child("最近文件"));
        match &self.recent_paths {
            Ok(paths) => {
                if paths.is_empty() {
                    panel = panel.child("打开或保存文稿后会显示在这里");
                }
                for (index, path) in paths.iter().enumerate() {
                    let path = path.clone();
                    let label = path
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_string();
                    let tip = path.to_string_lossy().to_string();
                    panel = panel.child(
                        Button::new(("recent-file", index))
                            .ghost()
                            .xsmall()
                            .label(label)
                            .tooltip(tip)
                            .h(px(26.))
                            .on_click(cx.listener(move |s, _, w, cx| {
                                s.open_path(path.clone(), w, cx);
                            })),
                    );
                }
            }
            Err(error) => panel = panel.child(format!("最近文件记录不可读取：{error}")),
        }
        panel
    }
    pub(super) fn templates_panel(&self, cx: &mut Context<Self>) -> Div {
        let mut panel = div()
            .flex()
            .flex_col()
            .gap_2()
            .child(div().text_lg().child("新建文稿"));
        let mut choices = div().flex().flex_wrap().gap_2();
        for name in ["简洁简报", "课程讲义", "项目计划"] {
            choices = choices.child(
                Button::new(name)
                    .ghost()
                    .label(name)
                    .w(px(150.))
                    .h(px(52.))
                    .border_1()
                    .on_click(cx.listener(move |s, _, w, cx| s.new_template(name, w, cx))),
            );
        }
        panel = panel.child(choices).child(
            Button::new("new-window")
                .ghost()
                .xsmall()
                .label("在新窗口中新建空白文稿")
                .on_click(cx.listener(|s, _, _, cx| {
                    match super::open_window(s.dark, (1440., 900.), cx) {
                        Ok((handle, view)) => super::new_document(&view, handle, cx),
                        Err(error) => {
                            s.message = format!("无法新建窗口：{error}");
                            cx.notify();
                        }
                    }
                })),
        );
        panel
    }
    pub(super) fn new_template(
        &mut self,
        name: &'static str,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.confirm_replace(document_ui::ReplaceDocument::Template(name), w, cx) {
            return;
        }
        let result = template_editor(name, joydeck_shell::editor::recovery_root());
        match result {
            Ok(editor) => {
                self.stop_read();
                self.attach_new_editor(editor, cx);
                self.page = 0;
                self.zoom = None;
                self.cell = None;
                self.cell_range = None;
                self.slide_selection.clear();
                self.slide_anchor = None;
                self.backstage = false;
                self.generation += 1;
                self.sync_editor(w, cx);
                self.checkpoint(cx);
                self.message = format!("已从“{name}”新建，请另存文稿");
            }
            Err(error) => self.message = error,
        }
        cx.notify();
    }
}

fn template_editor(
    name: &str,
    root: std::path::PathBuf,
) -> Result<joydeck_shell::editor::Editor, String> {
    let pages: &[(&str, &str)] = match name {
        "简洁简报" => &[
            ("项目简报", "主题 · 日期 · 汇报人"),
            ("关键结论", "结论一\n结论二\n下一步行动"),
            ("行动计划", "任务 · 负责人 · 完成时间"),
        ],
        "课程讲义" => &[
            ("课程主题", "学习目标与适用对象"),
            ("核心概念", "概念说明\n原创示例\n练习与讨论"),
            ("课程回顾", "重点总结\n课后练习"),
        ],
        "项目计划" => &[
            ("项目计划", "项目目标与预期成果"),
            ("里程碑", "启动\n实施\n验收"),
            ("风险与资源", "主要风险\n应对措施\n所需资源"),
        ],
        _ => return Err("未知模板".to_string()),
    };
    let mut editor = joydeck_shell::editor::Editor::new(root)?;
    let ids = editor.session.model().slides[0]
        .shapes
        .iter()
        .map(|s| s.id)
        .collect();
    editor.session.select(0, ids)?;
    editor.session.execute(Command::Delete)?;
    let mut commands = vec![Command::Content {
        action: ContentAction::ThemeAccent {
            slot: 1,
            rgb: if name == "课程讲义" {
                0x268270
            } else {
                0xd95a1a
            },
        },
    }];
    for (index, (title, body)) in pages.iter().enumerate() {
        if index > 0 {
            commands.push(Command::NewSlide);
        }
        commands.push(Command::InsertText {
            x: 72.,
            y: 70.,
            text: (*title).into(),
        });
        commands.push(Command::InsertText {
            x: 72.,
            y: 170.,
            text: (*body).into(),
        });
    }
    editor.session.execute(Command::Batch { commands })?;
    let snapshot = editor.session.snapshot();
    editor.session = Session::new(snapshot.model.as_ref().clone(), snapshot.origins, false);
    Ok(editor)
}

#[cfg(test)]
mod tests {
    use super::template_editor;
    #[test]
    fn starter_templates_create_save_and_reopen_all_three_pages() {
        let root = std::env::temp_dir().join(format!(
            "joydeck-template-roundtrip-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&root).unwrap();
        for (name, titles) in [
            ("简洁简报", ["项目简报", "关键结论", "行动计划"]),
            ("课程讲义", ["课程主题", "核心概念", "课程回顾"]),
            ("项目计划", ["项目计划", "里程碑", "风险与资源"]),
        ] {
            let mut editor = template_editor(name, root.join("recovery")).unwrap();
            assert!(editor.session.dirty());
            assert_eq!(editor.session.model().slides.len(), 3);
            for (slide, title) in editor.session.model().slides.iter().zip(titles) {
                assert!(
                    slide.shapes.iter().any(|shape| {
                        shape.text.as_ref().is_some_and(|text| text.text() == title)
                    })
                );
            }
            let path = root.join(format!("{name}.joydeck"));
            let saved = editor.save_job(path.clone()).unwrap().run().unwrap();
            editor.apply_saved(saved);
            assert!(!editor.session.dirty());
            let reopened =
                joydeck_shell::editor::Editor::open(&path, root.join("recovery")).unwrap();
            assert_eq!(
                serde_json::to_value(editor.session.model()).unwrap(),
                serde_json::to_value(reopened.session.model()).unwrap()
            );
            assert!(!reopened.session.dirty());
        }
        std::fs::remove_dir_all(root).unwrap();
    }
}
