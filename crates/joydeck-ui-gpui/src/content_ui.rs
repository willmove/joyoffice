//! Contextual R5 controls, using JoyOffice shared palette and GPUI Kit inputs/dialogs.
use super::*;
use joydeck_engine::{Command, ContentAction};
use joydeck_model::{ShapeKind, resolve};

#[derive(Clone, Copy)]
pub(super) enum Form {
    Background,
    MasterBackground,
    Accent,
    Fonts,
    Size,
    Picture,
    ChartValue,
    ChartTitle,
    Metadata,
    ShapeFill,
    ShapeGradient,
    ImportPicture(bool),
}
impl Viewer {
    pub(super) fn content_inspector(&self, cx: &mut Context<Self>) -> AnyElement {
        let p = chrome::palette(self.dark);
        let mut panel = div()
            .id("content-inspector")
            .w(px(264.))
            .h_full()
            .flex_shrink_0()
            .overflow_y_scroll()
            .p_4()
            .flex()
            .flex_col()
            .gap_3()
            .bg(rgb(p.surface))
            .border_l_1()
            .border_color(rgb(p.border))
            .text_sm();
        panel = panel.child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .child(div().text_lg().child("选择与格式"))
                .child(
                    Button::new("close-inspector")
                        .label("×")
                        .accessibility_label("关闭选择窗格")
                        .on_click(cx.listener(|s, _, _, cx| {
                            s.inspector = false;
                            cx.notify();
                        })),
                ),
        );
        if let Some(e) = &self.editor {
            let pres = e.session.model();
            let slide = &pres.slides[self.page];
            for shape in slide.shapes.iter().rev() {
                let id = shape.id;
                panel = panel.child(
                    Button::new(("object-row", id.0))
                        .label(shape.name.clone())
                        .w_full()
                        .on_click(cx.listener(move |s, _, w, cx| {
                            if s.composing() {
                                return;
                            }
                            s.end_inline(cx);
                            if let Some(e) = &mut s.editor {
                                let _ = e.session.select(s.page, vec![id]);
                            }
                            s.cell = None;
                            s.focus.focus(w, cx);
                            cx.notify();
                        })),
                );
            }
            panel = panel.child(
                Button::new("object-geometry")
                    .label("大小和位置…")
                    .disabled(e.session.selection().shapes.is_empty())
                    .on_click(cx.listener(|s, _, w, cx| s.geometry_dialog(w, cx))),
            );
            let layout = pres
                .layout(slide.layout)
                .map(|(m, l)| format!("{} / {}", m.name, l.name))
                .unwrap_or_default();
            panel = panel
                .child(format!(
                    "{:.1} × {:.1} pt",
                    pres.slide_size.width, pres.slide_size.height
                ))
                .child(layout);
            if let Some((m, _)) = pres.master_for(slide) {
                panel = panel
                    .child(format!("主题：{}", m.theme.name))
                    .child(format!("标题字体：{}", m.theme.fonts.major.latin))
                    .child(format!("正文字体：{}", m.theme.fonts.minor.latin));
            }
            for id in &e.session.selection().shapes {
                if let Some(s) = slide.shape(*id) {
                    panel = panel.child(div().font_weight(FontWeight::BOLD).child(s.name.clone()));
                    if let Some(ctx) = resolve::Ctx::for_slide(pres, slide) {
                        let x = resolve::xfrm(&ctx, s);
                        panel = panel.child(format!(
                            "位置 {:.1}, {:.1}\n大小 {:.1} × {:.1}\n旋转 {:.1}°",
                            x.x, x.y, x.w, x.h, x.rot
                        ));
                    }
                    let detail = match &s.kind {
                        ShapeKind::Picture { fill } => format!(
                            "图片 · media {}\n裁剪 {:?}\n透明度 {:.0}%",
                            fill.media.0,
                            fill.crop,
                            (1. - fill.alpha.unwrap_or(1.)) * 100.
                        ),
                        ShapeKind::Chart(c) => format!(
                            "图表 {}\n{} 个系列 / {} 个分类",
                            c.kind.label(),
                            c.series.len(),
                            c.categories.len()
                        ),
                        ShapeKind::Table(t) => {
                            format!("表格 {} 行 × {} 列", t.rows.len(), t.cols.len())
                        }
                        ShapeKind::Opaque { label, .. } => {
                            format!("复杂内容：{label}\n保留原始部件；当前只读")
                        }
                        ShapeKind::Group { children, .. } => {
                            format!("组合 · {} 个子对象", children.len())
                        }
                        _ => "形状 / 文字".into(),
                    };
                    panel = panel.child(detail);
                }
            }
            let shape_selected = e.session.selection().shapes.len() == 1
                && e.session
                    .selection()
                    .shapes
                    .first()
                    .and_then(|id| slide.shape(*id))
                    .is_some_and(|s| matches!(s.kind, ShapeKind::Shape));
            panel = panel.child(
                Button::new("shape-fill-form")
                    .label("形状填充颜色")
                    .disabled(!shape_selected)
                    .on_click(cx.listener(|s, _, w, cx| s.content_form(Form::ShapeFill, w, cx))),
            );
            panel = panel.child(
                Button::new("shape-gradient-form")
                    .label("渐变填充")
                    .disabled(!shape_selected)
                    .on_click(
                        cx.listener(|s, _, w, cx| s.content_form(Form::ShapeGradient, w, cx)),
                    ),
            );
            for (name, label) in [
                ("rect", "矩形"),
                ("roundRect", "圆角矩形"),
                ("ellipse", "椭圆"),
                ("triangle", "三角形"),
                ("chevron", "燕尾形"),
            ] {
                panel = panel.child(
                    Button::new(name)
                        .label(label)
                        .disabled(!shape_selected)
                        .on_click(cx.listener(move |s, _, w, cx| {
                            s.dispatch(
                                Command::Content {
                                    action: ContentAction::ShapePreset { name: name.into() },
                                },
                                w,
                                cx,
                            )
                        })),
                );
            }
        }
        panel.into_any_element()
    }
    pub(super) fn content_form(&mut self, form: Form, w: &mut Window, cx: &mut Context<Self>) {
        if self.composing() {
            self.message = "请先结束组合输入".into();
            cx.notify();
            return;
        }
        self.end_inline(cx);
        if let Form::ImportPicture(replace) = form {
            if self.editor.is_none() {
                return;
            }
            let receiver = cx.prompt_for_paths(PathPromptOptions {
                files: true,
                directories: false,
                multiple: false,
                prompt: Some(
                    if replace {
                        "替换图片"
                    } else {
                        "插入图片"
                    }
                    .into(),
                ),
            });
            let generation = self.generation;
            let selection = self
                .editor
                .as_ref()
                .map(|e| (e.session.revision(), e.session.selection().clone()));
            self.dialog_task = Some(cx.spawn(async move |view, cx| {
                if let Ok(Ok(Some(paths))) = receiver.await
                    && let Some(path) = paths.into_iter().next()
                {
                    let _ = view.update(cx, |s, cx| {
                        if s.generation == generation
                            && s.editor
                                .as_ref()
                                .map(|e| (e.session.revision(), e.session.selection()))
                                == selection
                                    .as_ref()
                                    .map(|(revision, selection)| (*revision, selection))
                        {
                            s.import_picture(path.to_string_lossy().into_owned(), replace, cx);
                        }
                    });
                }
            }));
            return;
        }
        let Some(e) = &self.editor else { return };
        let pres = e.session.model();
        let slide = &pres.slides[self.page];
        let shape = e
            .session
            .selection()
            .shapes
            .first()
            .and_then(|id| slide.shape(*id));
        let (title, fields): (String, Vec<(&str, String)>) = match form {
            Form::Background => ("页面背景".into(), vec![("RGB 十六进制", "F4F7FB".into())]),
            Form::MasterBackground => (
                "母版背景（影响继承此背景的页面）".into(),
                vec![("RGB 十六进制", "E8EFF8".into())],
            ),
            Form::Accent => (
                "当前母版的主题颜色".into(),
                vec![
                    ("强调色编号 1–6", "1".into()),
                    ("RGB 十六进制", "2678D0".into()),
                ],
            ),
            Form::Fonts => {
                let Some((m, _)) = pres.master_for(slide) else {
                    return;
                };
                (
                    "主题字体".into(),
                    vec![
                        ("标题字体", m.theme.fonts.major.latin.clone()),
                        ("正文字体", m.theme.fonts.minor.latin.clone()),
                    ],
                )
            }
            Form::Size => (
                "页面尺寸（不缩放现有对象）".into(),
                vec![
                    ("宽度 pt", pres.slide_size.width.to_string()),
                    ("高度 pt", pres.slide_size.height.to_string()),
                ],
            ),
            Form::Metadata => (
                "文稿属性".into(),
                vec![
                    ("标题", pres.props.title.clone()),
                    ("作者", pres.props.author.clone()),
                ],
            ),
            Form::ShapeFill => ("形状填充".into(), vec![("RGB 十六进制", "2678D0".into())]),
            Form::ShapeGradient => (
                "线性渐变填充".into(),
                vec![
                    ("起点 RGB", "2678D0".into()),
                    ("终点 RGB", "A8D8F0".into()),
                    ("角度 −360–360°", "45".into()),
                ],
            ),
            Form::ImportPicture(replace) => (
                if replace {
                    "替换所选图片"
                } else {
                    "插入本地图片"
                }
                .into(),
                vec![("图片文件路径（PNG / JPEG 等）", String::new())],
            ),
            Form::Picture => {
                let Some(joydeck_model::Shape {
                    kind: ShapeKind::Picture { fill },
                    ..
                }) = shape
                else {
                    self.message = "请先选择图片".into();
                    cx.notify();
                    return;
                };
                (
                    "图片裁剪与调整".into(),
                    vec![
                        ("左裁剪 0–0.95", fill.crop[0].to_string()),
                        ("上裁剪 0–0.95", fill.crop[1].to_string()),
                        ("右裁剪 0–0.95", fill.crop[2].to_string()),
                        ("下裁剪 0–0.95", fill.crop[3].to_string()),
                        ("不透明度 0–1", fill.alpha.unwrap_or(1.).to_string()),
                        ("亮度 −1–1", fill.adjust.brightness.to_string()),
                        ("对比度 −1–1", fill.adjust.contrast.to_string()),
                    ],
                )
            }
            Form::ChartTitle => {
                let Some(joydeck_model::Shape {
                    kind: ShapeKind::Chart(c),
                    ..
                }) = shape
                else {
                    self.message = "请先选择图表".into();
                    cx.notify();
                    return;
                };
                (
                    "图表标题".into(),
                    vec![("标题", c.title.clone().unwrap_or_default())],
                )
            }
            Form::ChartValue => {
                let Some(joydeck_model::Shape {
                    kind: ShapeKind::Chart(c),
                    ..
                }) = shape
                else {
                    self.message = "请先选择图表".into();
                    cx.notify();
                    return;
                };
                let mut grid = String::new();
                for (i, s) in c.series.iter().enumerate().take(8) {
                    grid.push_str(&format!(
                        "系列 {} {}：{} · ",
                        i + 1,
                        s.name,
                        s.values
                            .iter()
                            .take(8)
                            .map(|v| v.map(|n| n.to_string()).unwrap_or_else(|| "空".into()))
                            .collect::<Vec<_>>()
                            .join(" / ")
                    ));
                }
                (
                    format!(
                        "图表数据 · {} 系列 / {} 分类",
                        c.series.len(),
                        c.categories.len()
                    ),
                    vec![
                        ("系列编号（从 1 起）", "1".into()),
                        ("分类编号（从 1 起）", "1".into()),
                        (
                            "新值",
                            c.series
                                .first()
                                .and_then(|s| s.values.first())
                                .copied()
                                .flatten()
                                .unwrap_or(0.)
                                .to_string(),
                        ),
                        ("数据预览（前 8 项）", grid),
                    ],
                )
            }
        };
        let inputs: Vec<_> = fields
            .iter()
            .map(|(_, value)| {
                cx.new(|cx| {
                    let mut input = InputState::new(w, cx);
                    input.set_value(value.clone(), w, cx);
                    input
                })
            })
            .collect();
        let view = cx.entity().downgrade();
        let generation = self.generation;
        let selection = e.session.selection().clone();
        let revision = e.session.revision();
        let error = cx.new(|_| String::new());
        let form_height = (f32::from(w.viewport_size().height) - 240.).max(160.);
        w.open_dialog(cx, move |d, _, cx| {
            let mut content = div()
                .id("content-form-fields")
                .max_h(px(form_height))
                .overflow_y_scroll()
                .flex()
                .flex_col()
                .gap_2();
            for ((label, _), input) in fields.iter().zip(&inputs) {
                content = content
                    .child(div().text_sm().child(*label))
                    .child(Input::new(input).disabled(label.starts_with("数据预览")));
            }
            let error_label = error.read(cx).clone();
            let error = error.clone();
            let inputs = inputs.clone();
            let view = view.clone();
            let selection = selection.clone();
            d.title(title.clone())
                .child(content)
                .child(div().text_sm().text_color(rgb(0xcc4433)).child(error_label))
                .child(
                    Button::new("apply-content")
                        .label("应用")
                        .on_click(move |_, w, cx| {
                            if inputs
                                .iter()
                                .any(|i| i.update(cx, |s, cx| s.marked_text_range(w, cx).is_some()))
                            {
                                error.update(cx, |s, cx| {
                                    *s = "请先结束组合输入".into();
                                    cx.notify();
                                });
                                return;
                            }
                            let values: Vec<_> = inputs
                                .iter()
                                .map(|i| i.read(cx).value().to_string())
                                .collect();
                            let parsed = parse(form, &values);
                            let _ = view.update(cx, |s, cx| {
                                if s.generation != generation
                                    || s.editor.as_ref().is_none_or(|e| {
                                        e.session.revision() != revision
                                            || *e.session.selection() != selection
                                    })
                                {
                                    w.close_dialog(cx);
                                    return;
                                }
                                match parsed {
                                    Ok(Some(command)) => {
                                        w.close_dialog(cx);
                                        s.dispatch(command, w, cx);
                                    }
                                    Ok(None) => {
                                        w.close_dialog(cx);
                                        if let Form::ImportPicture(replace) = form {
                                            s.import_picture(values[0].clone(), replace, cx);
                                        }
                                    }
                                    Err(message) => error.update(cx, |s, cx| {
                                        *s = message;
                                        cx.notify();
                                    }),
                                }
                            });
                        }),
                )
        });
    }
    pub(super) fn paste_image(&mut self, bytes: Vec<u8>, cx: &mut Context<Self>) {
        let generation = self.generation;
        let selection = self
            .editor
            .as_ref()
            .map(|e| (e.session.revision(), e.session.selection().clone()));
        self.content_task = Some(cx.spawn(async move |view, cx| {
            let result = cx
                .background_executor()
                .spawn(async move { joydeck_shell::platform::picture_bytes_command(bytes, false) })
                .await;
            let _ = view.update(cx, |s, cx| {
                if s.generation != generation
                    || s.editor
                        .as_ref()
                        .map(|e| (e.session.revision(), e.session.selection()))
                        != selection
                            .as_ref()
                            .map(|(revision, selection)| (*revision, selection))
                {
                    return;
                }
                match result {
                    Ok(command) => {
                        if let Some(e) = &mut s.editor {
                            match e.execute(command, cx) {
                                Ok(true) => {
                                    s.refresh_pending = true;
                                    s.checkpoint(cx);
                                }
                                Ok(false) => {}
                                Err(e) => s.message = e,
                            }
                        }
                    }
                    Err(e) => s.message = e,
                }
                cx.notify();
            });
        }));
    }
    fn import_picture(&mut self, path: String, replace: bool, cx: &mut Context<Self>) {
        let generation = self.generation;
        let selection = self
            .editor
            .as_ref()
            .map(|e| (e.session.revision(), e.session.selection().clone()));
        self.content_task = Some(cx.spawn(async move |view, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    joydeck_shell::platform::picture_command(
                        std::path::Path::new(path.trim().trim_matches('"')),
                        replace,
                    )
                })
                .await;
            let _ = view.update(cx, |s, cx| {
                if s.generation != generation
                    || s.editor
                        .as_ref()
                        .map(|e| (e.session.revision(), e.session.selection()))
                        != selection
                            .as_ref()
                            .map(|(revision, selection)| (*revision, selection))
                {
                    return;
                }
                match result {
                    Ok(command) => {
                        if let Some(e) = &mut s.editor {
                            match e.execute(command, cx) {
                                Ok(true) => {
                                    s.refresh_pending = true;
                                    s.checkpoint(cx);
                                }
                                Ok(false) => {}
                                Err(e) => s.message = e,
                            }
                        }
                    }
                    Err(e) => s.message = e,
                }
                cx.notify();
            });
        }));
    }
}
fn parse(form: Form, v: &[String]) -> Result<Option<Command>, String> {
    let n = |i: usize| {
        v[i].trim()
            .parse::<f64>()
            .map_err(|_| "请输入有效数字".to_string())
    };
    let rgb = |i: usize| {
        u32::from_str_radix(v[i].trim().trim_start_matches('#'), 16)
            .map_err(|_| "请输入 RGB 十六进制颜色".to_string())
    };
    let index = |i: usize| {
        v[i].trim()
            .parse::<usize>()
            .ok()
            .and_then(|n| n.checked_sub(1))
            .ok_or_else(|| "编号从 1 起".to_string())
    };
    let action = match form {
        Form::Background => ContentAction::Background { rgb: rgb(0)? },
        Form::MasterBackground => ContentAction::MasterBackground { rgb: rgb(0)? },
        Form::Accent => ContentAction::ThemeAccent {
            slot: v[0].trim().parse().map_err(|_| "强调色编号为 1–6")?,
            rgb: rgb(1)?,
        },
        Form::Fonts => ContentAction::ThemeFonts {
            major: v[0].clone(),
            minor: v[1].clone(),
        },
        Form::Size => ContentAction::SlideSize {
            width: n(0)?,
            height: n(1)?,
        },
        Form::Metadata => ContentAction::Metadata {
            title: v[0].clone(),
            author: v[1].clone(),
        },
        Form::ShapeFill => ContentAction::ShapeFill { rgb: rgb(0)? },
        Form::ShapeGradient => ContentAction::ShapeGradient {
            start_rgb: rgb(0)?,
            end_rgb: rgb(1)?,
            angle: n(2)?,
        },
        Form::Picture => ContentAction::Picture {
            crop: [n(0)?, n(1)?, n(2)?, n(3)?],
            alpha: n(4)?,
            brightness: n(5)?,
            contrast: n(6)?,
        },
        Form::ChartTitle => ContentAction::ChartTitle {
            title: v[0].clone(),
        },
        Form::ChartValue => ContentAction::ChartValue {
            series: index(0)?,
            index: index(1)?,
            value: n(2)?,
        },
        Form::ImportPicture(_) => return Ok(None),
    };
    Ok(Some(Command::Content { action }))
}
