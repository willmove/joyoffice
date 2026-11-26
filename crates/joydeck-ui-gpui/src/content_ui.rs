//! Contextual R5 controls, using JoyOffice shared palette and GPUI Kit inputs/dialogs.
use super::*;
use crate::l10n::{presentation_theme_name, t, tf};
use joydeck_engine::{AlignEdge, Command, ContentAction, DistributeAxis};
use joydeck_model::{ShapeKind, resolve};
use joyoffice_ui::UiLocale;

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
    ShapeLine,
    TransitionDuration,
    ImportPicture(bool),
}
impl Viewer {
    pub(super) fn content_inspector(&self, cx: &mut Context<Self>) -> AnyElement {
        let locale = self.locale;
        let p = self.palette();
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
                .child(div().text_lg().child(t(locale, "pane.title")))
                .child(
                    Button::new("close-inspector")
                        .label("×")
                        .accessibility_label(t(locale, "pane.close_aria"))
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
                    .label(t(locale, "pane.size_pos_btn"))
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
                let theme_name = presentation_theme_name(locale, &m.theme.name);
                panel = panel
                    .child(tf(locale, "pane.theme", &[("name", &theme_name)]))
                    .child(tf(
                        locale,
                        "pane.major_font",
                        &[("font", &m.theme.fonts.major.latin)],
                    ))
                    .child(tf(
                        locale,
                        "pane.minor_font",
                        &[("font", &m.theme.fonts.minor.latin)],
                    ));
            }
            for id in &e.session.selection().shapes {
                if let Some(s) = slide.shape(*id) {
                    panel = panel.child(div().font_weight(FontWeight::BOLD).child(s.name.clone()));
                    if let Some(ctx) = resolve::Ctx::for_slide(pres, slide) {
                        let x = resolve::xfrm(&ctx, s);
                        let x_str = format!("{:.1}", x.x);
                        let y_str = format!("{:.1}", x.y);
                        let w_str = format!("{:.1}", x.w);
                        let h_str = format!("{:.1}", x.h);
                        let rot_str = format!("{:.1}", x.rot);
                        panel = panel.child(tf(
                            locale,
                            "pane.geom_summary",
                            &[
                                ("x", &x_str),
                                ("y", &y_str),
                                ("w", &w_str),
                                ("h", &h_str),
                                ("rot", &rot_str),
                            ],
                        ));
                    }
                    let detail = match &s.kind {
                        ShapeKind::Picture { fill } => {
                            let media = fill.media.0.to_string();
                            let crop = format!("{:?}", fill.crop);
                            let alpha = format!("{:.0}", (1. - fill.alpha.unwrap_or(1.)) * 100.);
                            tf(
                                locale,
                                "pane.picture_summary",
                                &[("media", &media), ("crop", &crop), ("alpha", &alpha)],
                            )
                        }
                        ShapeKind::Chart(c) => {
                            let series = c.series.len().to_string();
                            let cats = c.categories.len().to_string();
                            tf(
                                locale,
                                "pane.chart_summary",
                                &[
                                    ("kind", c.kind.label()),
                                    ("series", &series),
                                    ("cats", &cats),
                                ],
                            )
                        }
                        ShapeKind::Table(tbl) => {
                            let rows = tbl.rows.len().to_string();
                            let cols = tbl.cols.len().to_string();
                            tf(
                                locale,
                                "pane.table_summary",
                                &[("rows", &rows), ("cols", &cols)],
                            )
                        }
                        ShapeKind::Opaque { label, .. } => {
                            tf(locale, "pane.complex_summary", &[("label", label)])
                        }
                        ShapeKind::Group { children, .. } => {
                            let count = children.len().to_string();
                            tf(locale, "pane.group_summary", &[("count", &count)])
                        }
                        _ => t(locale, "pane.shape_text").into(),
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
                    .label(t(locale, "pane.shape_fill_color"))
                    .disabled(!shape_selected)
                    .on_click(cx.listener(|s, _, w, cx| s.content_form(Form::ShapeFill, w, cx))),
            );
            panel = panel.child(
                Button::new("shape-gradient-form")
                    .label(t(locale, "shape.gradient_fill"))
                    .disabled(!shape_selected)
                    .on_click(
                        cx.listener(|s, _, w, cx| s.content_form(Form::ShapeGradient, w, cx)),
                    ),
            );
            panel = panel.child(
                Button::new("shape-line-form")
                    .label(t(locale, "home.shape_outline"))
                    .disabled(!self.can_edit_outline())
                    .on_click(cx.listener(|s, _, w, cx| s.content_form(Form::ShapeLine, w, cx))),
            );
            for (name, label) in [
                ("rect", t(locale, "shapes.rect")),
                ("roundRect", t(locale, "shapes.round_rect")),
                ("ellipse", t(locale, "shapes.ellipse")),
                ("triangle", t(locale, "shapes.triangle_short")),
                ("chevron", t(locale, "shapes.chevron")),
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
            panel = panel.child(
                Button::new("shape-preset-more")
                    .label(t(locale, "shapes.more_btn"))
                    .disabled(!shape_selected)
                    .on_click(cx.listener(|s, _, w, cx| s.shape_preset_gallery(w, cx))),
            );
        }
        panel.into_any_element()
    }
    pub(super) fn content_form(&mut self, form: Form, w: &mut Window, cx: &mut Context<Self>) {
        let locale = self.locale;
        if self.composing() {
            self.message = t(locale, "msg.finish_ime_edit").into();
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
                        t(locale, "form.replace_picture")
                    } else {
                        t(locale, "form.insert_picture")
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
        let preview_label = t(locale, "form.chart_data_preview");
        let (title, fields): (String, Vec<(&'static str, String)>) = match form {
            Form::Background => (
                t(locale, "form.slide_bg").into(),
                vec![(t(locale, "form.rgb_hex"), "F4F7FB".into())],
            ),
            Form::MasterBackground => (
                t(locale, "form.master_bg").into(),
                vec![(t(locale, "form.rgb_hex"), "E8EFF8".into())],
            ),
            Form::Accent => (
                t(locale, "form.master_accent").into(),
                vec![
                    (t(locale, "form.accent_slot"), "1".into()),
                    (t(locale, "form.rgb_hex"), "2678D0".into()),
                ],
            ),
            Form::Fonts => {
                let Some((m, _)) = pres.master_for(slide) else {
                    return;
                };
                (
                    t(locale, "form.theme_fonts").into(),
                    vec![
                        (
                            t(locale, "form.major_font_field"),
                            m.theme.fonts.major.latin.clone(),
                        ),
                        (
                            t(locale, "form.minor_font_field"),
                            m.theme.fonts.minor.latin.clone(),
                        ),
                    ],
                )
            }
            Form::Size => (
                t(locale, "form.slide_size").into(),
                vec![
                    (
                        t(locale, "form.width_pt"),
                        pres.slide_size.width.to_string(),
                    ),
                    (
                        t(locale, "form.height_pt"),
                        pres.slide_size.height.to_string(),
                    ),
                ],
            ),
            Form::Metadata => (
                t(locale, "form.doc_props").into(),
                vec![
                    (t(locale, "form.prop_title"), pres.props.title.clone()),
                    (t(locale, "form.prop_author"), pres.props.author.clone()),
                ],
            ),
            Form::ShapeFill => (
                t(locale, "form.shape_fill").into(),
                vec![(t(locale, "form.rgb_hex"), "2678D0".into())],
            ),
            Form::ShapeGradient => (
                t(locale, "form.linear_gradient").into(),
                vec![
                    (t(locale, "form.start_rgb"), "2678D0".into()),
                    (t(locale, "form.end_rgb"), "A8D8F0".into()),
                    (t(locale, "form.angle_deg"), "45".into()),
                ],
            ),
            Form::ShapeLine => {
                let current_line = shape.and_then(|s| s.line.as_ref());
                let hex = current_line
                    .and_then(|l| l.fill.as_ref())
                    .and_then(|f| match f {
                        joydeck_model::Fill::Solid { color } => match color.base {
                            joydeck_model::style::ColorBase::Rgb { rgb } => Some(rgb.hex()),
                            _ => None,
                        },
                        _ => None,
                    })
                    .unwrap_or_else(|| "2678D0".into());
                let width = current_line
                    .and_then(|l| l.width)
                    .map(|w| format!("{w:.2}"))
                    .unwrap_or_else(|| "1.50".into());
                let dash = current_line
                    .and_then(|l| l.dash.as_ref())
                    .and_then(|d| match d {
                        joydeck_model::style::Dash::Preset { name } => Some(name.clone()),
                        _ => None,
                    })
                    .unwrap_or_else(|| "solid".into());
                (
                    t(locale, "form.shape_outline_title").into(),
                    vec![
                        (t(locale, "form.outline_rgb"), hex),
                        (t(locale, "form.outline_width"), width),
                        (t(locale, "form.outline_dash"), dash),
                    ],
                )
            }
            Form::TransitionDuration => {
                let seconds = slide
                    .transition
                    .as_ref()
                    .map_or(1.0, |t| f64::from(t.duration_ms) / 1000.0);
                (
                    t(locale, "form.transition_duration").into(),
                    vec![(
                        t(locale, "form.duration_seconds_field"),
                        format!("{seconds:.2}"),
                    )],
                )
            }
            Form::ImportPicture(replace) => (
                if replace {
                    t(locale, "form.replace_picture_title")
                } else {
                    t(locale, "form.insert_local_picture")
                }
                .into(),
                vec![(t(locale, "form.image_path_field"), String::new())],
            ),
            Form::Picture => {
                let Some(joydeck_model::Shape {
                    kind: ShapeKind::Picture { fill },
                    ..
                }) = shape
                else {
                    self.message = t(locale, "form.err_select_picture").into();
                    cx.notify();
                    return;
                };
                (
                    t(locale, "form.picture_adjust_title").into(),
                    vec![
                        (t(locale, "form.crop_left"), fill.crop[0].to_string()),
                        (t(locale, "form.crop_top"), fill.crop[1].to_string()),
                        (t(locale, "form.crop_right"), fill.crop[2].to_string()),
                        (t(locale, "form.crop_bottom"), fill.crop[3].to_string()),
                        (
                            t(locale, "form.opacity_field"),
                            fill.alpha.unwrap_or(1.).to_string(),
                        ),
                        (
                            t(locale, "form.brightness_field"),
                            fill.adjust.brightness.to_string(),
                        ),
                        (
                            t(locale, "form.contrast_field"),
                            fill.adjust.contrast.to_string(),
                        ),
                    ],
                )
            }
            Form::ChartTitle => {
                let Some(joydeck_model::Shape {
                    kind: ShapeKind::Chart(c),
                    ..
                }) = shape
                else {
                    self.message = t(locale, "form.err_select_chart").into();
                    cx.notify();
                    return;
                };
                (
                    t(locale, "form.chart_title").into(),
                    vec![(
                        t(locale, "form.prop_title"),
                        c.title.clone().unwrap_or_default(),
                    )],
                )
            }
            Form::ChartValue => {
                let Some(joydeck_model::Shape {
                    kind: ShapeKind::Chart(c),
                    ..
                }) = shape
                else {
                    self.message = t(locale, "form.err_select_chart").into();
                    cx.notify();
                    return;
                };
                let mut grid = String::new();
                let empty_label = t(locale, "common.empty");
                for (i, s) in c.series.iter().enumerate().take(8) {
                    let num = (i + 1).to_string();
                    let values = s
                        .values
                        .iter()
                        .take(8)
                        .map(|v| {
                            v.map(|n| n.to_string())
                                .unwrap_or_else(|| empty_label.into())
                        })
                        .collect::<Vec<_>>()
                        .join(" / ");
                    grid.push_str(&tf(
                        locale,
                        "form.chart_series_preview",
                        &[("num", &num), ("name", &s.name), ("values", &values)],
                    ));
                }
                let series_count = c.series.len().to_string();
                let cats_count = c.categories.len().to_string();
                (
                    tf(
                        locale,
                        "form.chart_data_title",
                        &[("series", &series_count), ("cats", &cats_count)],
                    ),
                    vec![
                        (t(locale, "form.chart_series_idx"), "1".into()),
                        (t(locale, "form.chart_cat_idx"), "1".into()),
                        (
                            t(locale, "form.chart_new_val"),
                            c.series
                                .first()
                                .and_then(|s| s.values.first())
                                .copied()
                                .flatten()
                                .unwrap_or(0.)
                                .to_string(),
                        ),
                        (preview_label, grid),
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
                    .child(Input::new(input).disabled(*label == preview_label));
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
                        .label(t(locale, "common.apply"))
                        .on_click(move |_, w, cx| {
                            if inputs
                                .iter()
                                .any(|i| i.update(cx, |s, cx| s.marked_text_range(w, cx).is_some()))
                            {
                                error.update(cx, |s, cx| {
                                    *s = t(locale, "msg.finish_ime_edit").into();
                                    cx.notify();
                                });
                                return;
                            }
                            let values: Vec<_> = inputs
                                .iter()
                                .map(|i| i.read(cx).value().to_string())
                                .collect();
                            let parsed = parse(locale, form, &values);
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
                                        } else if let Form::TransitionDuration = form
                                            && let Ok(sec) = values[0].trim().parse::<f64>()
                                        {
                                            let ms = (sec.clamp(0.0, 60.0) * 1000.0).round() as u32;
                                            s.set_transition_duration(ms, w, cx);
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

    pub(super) fn can_edit_outline(&self) -> bool {
        if self.loading || self.composing() {
            return false;
        }
        let Some(e) = &self.editor else { return false };
        let sel = &e.session.selection().shapes;
        if sel.len() != 1 {
            return false;
        }
        e.session
            .model()
            .slides
            .get(self.page)
            .and_then(|slide| slide.shape(sel[0]))
            .is_some_and(|s| {
                !s.locked && matches!(s.kind, ShapeKind::Shape | ShapeKind::Connector { .. })
            })
    }

    pub(super) fn can_align(&self) -> bool {
        if self.loading || self.composing() {
            return false;
        }
        let Some(e) = &self.editor else { return false };
        let sel = &e.session.selection().shapes;
        !sel.is_empty()
            && e.session
                .model()
                .slides
                .get(self.page)
                .is_some_and(|slide| {
                    sel.iter()
                        .all(|id| slide.shape(*id).is_some_and(|s| !s.locked))
                })
    }

    pub(super) fn can_distribute(&self) -> bool {
        self.can_align()
            && self
                .editor
                .as_ref()
                .is_some_and(|e| e.session.selection().shapes.len() >= 3)
    }

    pub(super) fn can_group(&self) -> bool {
        self.can_align()
            && self
                .editor
                .as_ref()
                .is_some_and(|e| e.session.selection().shapes.len() >= 2)
    }

    pub(super) fn can_ungroup(&self) -> bool {
        if self.loading || self.composing() {
            return false;
        }
        let Some(e) = &self.editor else { return false };
        let sel = &e.session.selection().shapes;
        if sel.len() != 1 {
            return false;
        }
        e.session
            .model()
            .slides
            .get(self.page)
            .and_then(|slide| slide.shape(sel[0]))
            .is_some_and(|s| {
                !s.locked
                    && s.diagram.is_none()
                    && matches!(&s.kind, ShapeKind::Group { children, .. } if !children.is_empty())
            })
    }

    pub(super) fn set_shape_line(
        &mut self,
        rgb: Option<u32>,
        width: Option<f64>,
        dash: Option<String>,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.dispatch(
            Command::Content {
                action: ContentAction::ShapeLine { rgb, width, dash },
            },
            w,
            cx,
        );
    }

    pub(super) fn align_selected(
        &mut self,
        edge: AlignEdge,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.dispatch(
            Command::Content {
                action: ContentAction::AlignObjects { edge },
            },
            w,
            cx,
        );
    }

    pub(super) fn distribute_selected(
        &mut self,
        axis: DistributeAxis,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.dispatch(
            Command::Content {
                action: ContentAction::DistributeObjects { axis },
            },
            w,
            cx,
        );
    }

    pub(super) fn group_selected(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        self.dispatch(
            Command::Content {
                action: ContentAction::GroupObjects,
            },
            w,
            cx,
        );
    }

    pub(super) fn ungroup_selected(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        self.dispatch(
            Command::Content {
                action: ContentAction::UngroupObjects,
            },
            w,
            cx,
        );
    }

    fn current_transition_parts(&self) -> (String, String, u32, bool, Option<u32>) {
        if let Some(t) = self.current_slide().and_then(|s| s.transition.as_ref()) {
            (
                t.kind.clone(),
                t.option.clone(),
                t.duration_ms,
                t.advance_on_click,
                t.advance_after_ms,
            )
        } else {
            ("none".into(), String::new(), 1000, true, None)
        }
    }

    pub(super) fn set_transition_effect(
        &mut self,
        effect: &str,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let (_, _, _, advance_on_click, advance_after_ms) = self.current_transition_parts();
        let (default_ms, default_opt) = joydeck_model::anim::TRANSITIONS
            .iter()
            .find(|(id, _, _, _, _)| *id == effect)
            .map(|(_, _, _, ms, opts)| (*ms, opts.first().copied().unwrap_or("")))
            .unwrap_or((1000, ""));
        self.dispatch(
            Command::Content {
                action: ContentAction::SlideTransition {
                    effect: effect.into(),
                    option: default_opt.into(),
                    duration_ms: default_ms,
                    advance_on_click,
                    advance_after_ms,
                },
            },
            w,
            cx,
        );
    }

    pub(super) fn set_transition_option(
        &mut self,
        option: &str,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let (effect, _, duration_ms, advance_on_click, advance_after_ms) =
            self.current_transition_parts();
        self.dispatch(
            Command::Content {
                action: ContentAction::SlideTransition {
                    effect,
                    option: option.into(),
                    duration_ms,
                    advance_on_click,
                    advance_after_ms,
                },
            },
            w,
            cx,
        );
    }

    pub(super) fn set_transition_duration(
        &mut self,
        duration_ms: u32,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let (effect, option, _, advance_on_click, advance_after_ms) =
            self.current_transition_parts();
        self.dispatch(
            Command::Content {
                action: ContentAction::SlideTransition {
                    effect,
                    option,
                    duration_ms,
                    advance_on_click,
                    advance_after_ms,
                },
            },
            w,
            cx,
        );
    }

    pub(super) fn toggle_transition_on_click(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        let (effect, option, duration_ms, advance_on_click, advance_after_ms) =
            self.current_transition_parts();
        self.dispatch(
            Command::Content {
                action: ContentAction::SlideTransition {
                    effect,
                    option,
                    duration_ms,
                    advance_on_click: !advance_on_click,
                    advance_after_ms,
                },
            },
            w,
            cx,
        );
    }

    pub(super) fn set_transition_after(
        &mut self,
        advance_after_ms: Option<u32>,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let (effect, option, duration_ms, advance_on_click, _) = self.current_transition_parts();
        self.dispatch(
            Command::Content {
                action: ContentAction::SlideTransition {
                    effect,
                    option,
                    duration_ms,
                    advance_on_click,
                    advance_after_ms,
                },
            },
            w,
            cx,
        );
    }

    pub(super) fn apply_transition_all(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        self.dispatch(
            Command::Content {
                action: ContentAction::ApplyTransitionAll,
            },
            w,
            cx,
        );
    }
}
fn parse(locale: UiLocale, form: Form, v: &[String]) -> Result<Option<Command>, String> {
    let n = |i: usize| {
        v[i].trim()
            .parse::<f64>()
            .map_err(|_| t(locale, "form.err_valid_number").to_string())
    };
    let rgb = |i: usize| {
        u32::from_str_radix(v[i].trim().trim_start_matches('#'), 16)
            .map_err(|_| t(locale, "form.err_valid_rgb").to_string())
    };
    let index = |i: usize| {
        v[i].trim()
            .parse::<usize>()
            .ok()
            .and_then(|n| n.checked_sub(1))
            .ok_or_else(|| t(locale, "form.err_index_one").to_string())
    };
    let action = match form {
        Form::Background => ContentAction::Background { rgb: rgb(0)? },
        Form::MasterBackground => ContentAction::MasterBackground { rgb: rgb(0)? },
        Form::Accent => ContentAction::ThemeAccent {
            slot: v[0]
                .trim()
                .parse()
                .map_err(|_| t(locale, "form.err_accent_slot"))?,
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
        Form::ShapeLine => {
            let raw = v[0].trim();
            if raw.is_empty() {
                ContentAction::ShapeLine {
                    rgb: None,
                    width: None,
                    dash: None,
                }
            } else {
                let dash = v[2].trim();
                ContentAction::ShapeLine {
                    rgb: Some(rgb(0)?),
                    width: Some(n(1)?),
                    dash: if dash.is_empty() {
                        None
                    } else {
                        Some(dash.into())
                    },
                }
            }
        }
        Form::TransitionDuration => {
            let seconds = n(0)?;
            if !(0.0..=60.0).contains(&seconds) {
                return Err(t(locale, "form.err_duration_range").into());
            }
            return Ok(None);
        }
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
