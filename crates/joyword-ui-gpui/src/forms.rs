//! JoyOffice's version-bound native field forms, shared by authoring tools.
use super::*;
use gpui::component::{
    Sizable, WindowExt,
    input::{Input, InputState, Textarea, TextareaState},
};
use gpui::{Entity, EntityInputHandler, Focusable};
use serde_json::Value;
use std::rc::Rc;
pub(super) fn show(v: &Value) -> String {
    if let Some(s) = v.as_str() {
        s.into()
    } else if let Some(n) = v.as_f64() {
        format!("{n:.3}")
            .trim_end_matches('0')
            .trim_end_matches('.')
            .into()
    } else {
        String::new()
    }
}
/// A displayed choice keeps its engine value out of the user's input.
#[derive(Clone)]
pub(super) struct Choice {
    pub value: String,
    pub title: SharedString,
}
impl gpui::component::select::SelectItem for Choice {
    type Value = String;
    fn value(&self) -> &String {
        &self.value
    }
    fn title(&self) -> SharedString {
        self.title.clone()
    }
    fn render(&self, _: &mut Window, _: &mut App) -> impl IntoElement {
        div()
            .id(SharedString::from(self.value.clone()))
            .role(gpui::accesskit::Role::Label)
            .aria_label(self.title.clone())
            .child(self.title.clone())
    }
}
#[derive(Clone)]
pub(super) enum FieldKind {
    Text,
    Choice(Vec<Choice>),
    Check { yes: String, no: String },
    Length(joyword_shell::formatting::Unit),
    Color,
    Multiline,
}
#[derive(Clone)]
pub(super) struct Field {
    pub label: String,
    pub value: String,
    pub kind: FieldKind,
    pub tab: Option<&'static str>,
}
impl From<(&'static str, String)> for Field {
    fn from((label, value): (&'static str, String)) -> Self {
        Self::text(label, value)
    }
}
impl Field {
    pub(super) fn text(label: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            value: value.into(),
            kind: FieldKind::Text,
            tab: None,
        }
    }
    pub(super) fn choice(
        label: impl Into<String>,
        value: impl Into<String>,
        items: &[(&str, &str)],
    ) -> Self {
        let value = value.into();
        let mut items: Vec<_> = items
            .iter()
            .map(|(value, title)| Choice {
                value: (*value).into(),
                title: (*title).to_owned().into(),
            })
            .collect();
        if !value.is_empty() && !items.iter().any(|c| c.value == value) {
            items.insert(
                0,
                Choice {
                    title: format!("当前值：{value}").into(),
                    value: value.clone(),
                },
            );
        }
        Self {
            label: label.into(),
            value,
            kind: FieldKind::Choice(items),
            tab: None,
        }
    }
    pub(super) fn check(
        label: impl Into<String>,
        value: impl Into<String>,
        yes: &str,
        no: &str,
    ) -> Self {
        Self {
            label: label.into(),
            value: value.into(),
            kind: FieldKind::Check {
                yes: yes.into(),
                no: no.into(),
            },
            tab: None,
        }
    }
    pub(super) fn length(
        label: impl Into<String>,
        points: impl Into<String>,
        unit: joyword_shell::formatting::Unit,
    ) -> Self {
        Self {
            label: label.into(),
            value: points.into(),
            kind: FieldKind::Length(unit),
            tab: None,
        }
    }
    pub(super) fn color(label: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            value: value.into(),
            kind: FieldKind::Color,
            tab: None,
        }
    }
    pub(super) fn font(label: impl Into<String>, value: impl Into<String>) -> Self {
        let value = value.into();
        let mut names = joyword_shell::formatting::font_families();
        if !value.is_empty() && !names.contains(&value) {
            names.insert(0, value.clone());
        }
        Self {
            label: label.into(),
            value,
            kind: FieldKind::Choice(
                names
                    .into_iter()
                    .map(|name| Choice {
                        title: name.clone().into(),
                        value: name,
                    })
                    .collect(),
            ),
            tab: None,
        }
    }
    pub(super) fn multiline(label: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            value: value.into(),
            kind: FieldKind::Multiline,
            tab: None,
        }
    }
    pub(super) fn in_tab(mut self, tab: &'static str) -> Self {
        self.tab = Some(tab);
        self
    }
    fn displayed(&self) -> String {
        match self.kind {
            FieldKind::Color if self.value == "auto" => "自动".into(),
            FieldKind::Length(unit) => self
                .value
                .parse::<f32>()
                .ok()
                .filter(|n| n.is_finite())
                .map(|n| unit.number(n, '.'))
                .unwrap_or_default(),
            _ => self.value.clone(),
        }
    }
    fn submitted(&self, value: String, changed_only: bool) -> Result<String, String> {
        // An untouched rounded measurement must never rewrite the document's precise value.
        if value.trim() == self.displayed().trim() {
            return Ok(if changed_only {
                String::new()
            } else {
                self.value.clone()
            });
        }
        if value.trim().is_empty() {
            return Ok(String::new());
        }
        match self.kind {
            FieldKind::Color if value.trim() == "自动" => Ok("auto".into()),
            FieldKind::Length(unit) => joyword_shell::formatting::parse_measure(&value, unit)
                .map(|n| n.to_string())
                .ok_or_else(|| {
                    format!(
                        "{}：请输入有效尺寸（可用厘米、英寸或磅的单位缩写）",
                        self.label
                    )
                }),
            _ => Ok(value),
        }
    }
}
#[derive(Clone)]
enum FieldState {
    Text(Entity<InputState>),
    Choice(Entity<gpui::component::select::SelectState<Vec<Choice>>>),
    Multiline(Entity<TextareaState>),
    Check(Entity<Option<bool>>),
}
impl FieldState {
    fn value(&self, spec: &Field, cx: &App) -> String {
        match self {
            Self::Text(s) => s.read(cx).value().to_string(),
            Self::Multiline(s) => s.read(cx).value().to_string(),
            Self::Choice(s) => s.read(cx).selected_value().cloned().unwrap_or_default(),
            Self::Check(s) => match (&spec.kind, s.read(cx)) {
                (FieldKind::Check { yes, .. }, Some(true)) => yes.clone(),
                (FieldKind::Check { no, .. }, Some(false)) => no.clone(),
                _ => String::new(),
            },
        }
    }
}
impl JoyWord {
    pub(super) fn try_edit(
        &mut self,
        id: &str,
        args: &Value,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<Value, String> {
        if !self.format_ready(id) {
            return Err(self
                .canvas
                .reader
                .as_ref()
                .and_then(|r| r.formatting_disabled(id))
                .unwrap_or("编辑暂不可用")
                .into());
        }
        let mut result = Err("文档当前不可编辑".into());
        self.apply_input(w, cx, |r| {
            result = r.execute(id, args);
            result.as_ref().map(|_| ()).map_err(Clone::clone)
        });
        if result.is_ok() {
            self.advanced.key = None;
            w.focus(&self.focus, cx);
        }
        result
    }
    #[allow(clippy::too_many_arguments)]
    pub(super) fn fields_form<F: Into<Field>>(
        &mut self,
        title: &'static str,
        command: &'static str,
        fields: Vec<F>,
        changed_only: bool,
        parse: impl Fn(&[String]) -> Result<Value, String> + 'static,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.fields_action(
            title,
            command,
            fields,
            changed_only,
            parse,
            move |v, args, w, cx| v.try_edit(command, args, w, cx),
            w,
            cx,
        );
    }
    #[allow(clippy::too_many_arguments)]
    pub(super) fn fields_action<F: Into<Field>>(
        &mut self,
        title: &'static str,
        command: &'static str,
        fields: Vec<F>,
        changed_only: bool,
        parse: impl Fn(&[String]) -> Result<Value, String> + 'static,
        apply: impl Fn(&mut Self, &Value, &mut Window, &mut Context<Self>) -> Result<Value, String>
        + 'static,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.format_ready(command) {
            return;
        }
        let Some(r) = &self.canvas.reader else {
            return;
        };
        let target = (
            r.engine().document_id(),
            r.engine().rev(),
            r.presentation_revision(),
            r.engine().sel.clone(),
        );
        let fields: Vec<Field> = fields.into_iter().map(Into::into).collect();
        let inputs: Vec<_> = fields
            .into_iter()
            .map(|field| {
                let state = match &field.kind {
                    FieldKind::Multiline => FieldState::Multiline(cx.new(|cx| {
                        let mut s = TextareaState::new(w, cx).auto_grow(3, 6);
                        s.set_value(field.displayed(), w, cx);
                        s
                    })),
                    FieldKind::Choice(items) => FieldState::Choice(cx.new(|cx| {
                        let mut state =
                            gpui::component::select::SelectState::new(items.clone(), None, w, cx)
                                .searchable(items.len() > 12);
                        state.set_selected_value(&field.value, w, cx);
                        state
                    })),
                    FieldKind::Check { yes, .. } => FieldState::Check(cx.new(|_| {
                        if field.value.is_empty() {
                            None
                        } else {
                            Some(&field.value == yes)
                        }
                    })),
                    _ => FieldState::Text(cx.new(|cx| {
                        let mut state = InputState::new(w, cx).placeholder("多种值 / 未设置");
                        state.set_value(field.displayed(), w, cx);
                        state
                    })),
                };
                (field, state)
            })
            .collect();
        let view = cx.entity().downgrade();
        let mut tabs = Vec::new();
        for (field, _) in &inputs {
            if let Some(tab) = field.tab
                && !tabs.contains(&tab)
            {
                tabs.push(tab);
            }
        }
        let active_tab = cx.new(|_| tabs.first().copied());
        let error = cx.new(|_| String::new());
        let parse = Rc::new(parse);
        let apply = Rc::new(apply);
        w.open_dialog(cx, move |d, w, cx| {
            let mut body = div()
                .id("authoring-form-fields")
                .aria_label(title)
                .flex()
                .flex_col()
                .gap_2()
                .max_h(px((f32::from(w.viewport_size().height)
                    - if command == "format.set" { 280. } else { 190. })
                .clamp(120., 480.)))
                .overflow_y_scroll();
            if !tabs.is_empty() {
                body = body.child(
                    div()
                        .id("settings-tabs")
                        .flex()
                        .gap_1()
                        .role(gpui::Role::TabList)
                        .children(tabs.iter().enumerate().map(|(i, &tab)| {
                            let state = active_tab.clone();
                            div()
                                .id(("settings-tab", i))
                                .tab_stop(true)
                                .role(gpui::Role::Tab)
                                .aria_label(tab)
                                .aria_selected(*active_tab.read(cx) == Some(tab))
                                .px_2()
                                .py_1()
                                .rounded(px(4.))
                                .cursor_pointer()
                                .when(*active_tab.read(cx) == Some(tab), |el| el.bg(rgb(0xe2e8f0)))
                                .child(tab)
                                .on_key_down({
                                    let state = state.clone();
                                    move |e, w, cx| {
                                        if matches!(e.keystroke.key.as_str(), "enter" | "space") {
                                            state.update(cx, |v, cx| {
                                                *v = Some(tab);
                                                cx.notify();
                                            });
                                            w.prevent_default();
                                            cx.stop_propagation();
                                        }
                                    }
                                })
                                .on_click(move |_, _, cx| {
                                    state.update(cx, |v, cx| {
                                        *v = Some(tab);
                                        cx.notify();
                                    })
                                })
                        })),
                );
            }
            for (index, (field, input)) in inputs
                .iter()
                .enumerate()
                .filter(|(_, (field, _))| field.tab.is_none() || field.tab == *active_tab.read(cx))
            {
                let label = field.label.clone();
                match input {
                    FieldState::Text(input) => {
                        if matches!(field.kind, FieldKind::Color) {
                            let mut palette = div().flex().flex_wrap().gap_1();
                            for (color, name) in [
                                ("auto", "自动"),
                                ("000000", "黑色"),
                                ("FFFFFF", "白色"),
                                ("C00000", "深红色"),
                                ("FF0000", "红色"),
                                ("FFC000", "橙色"),
                                ("FFFF00", "黄色"),
                                ("70AD47", "绿色"),
                                ("00B0F0", "浅蓝色"),
                                ("0070C0", "蓝色"),
                                ("7030A0", "紫色"),
                                ("808080", "灰色"),
                            ] {
                                let input = input.clone();
                                palette = palette.child(
                                    Button::new(SharedString::from(format!(
                                        "form-color-{index}-{color}"
                                    )))
                                    .ghost()
                                    .small()
                                    .w(px(30.))
                                    .h(px(26.))
                                    .accessibility_label(format!("{}：{name}", field.label))
                                    .tooltip(name)
                                    .child(
                                        div()
                                            .size(px(17.))
                                            .border_1()
                                            .border_color(rgb(0x808080))
                                            .bg(rgb(
                                                u32::from_str_radix(color, 16).unwrap_or(0x222222)
                                            )),
                                    )
                                    .on_click(
                                        move |_, w, cx| {
                                            input.update(cx, |s, cx| {
                                                s.set_value(
                                                    if color == "auto" { "自动" } else { color },
                                                    w,
                                                    cx,
                                                )
                                            })
                                        },
                                    ),
                                );
                            }
                            body = body
                                .child(div().text_sm().child(label.clone()))
                                .child(palette)
                                .child(
                                    Input::new(input)
                                        .aria_label(format!("{label}（自定义RGB或自动）"))
                                        .small(),
                                );
                            continue;
                        }
                        let editor = Input::new(input).aria_label(label.clone()).small();
                        let editor = match field.kind {
                            FieldKind::Length(unit) => {
                                editor.suffix(div().text_sm().child(match unit {
                                    joyword_shell::formatting::Unit::Centimeters => "厘米",
                                    joyword_shell::formatting::Unit::Millimeters => "毫米",
                                    joyword_shell::formatting::Unit::Inches => "英寸",
                                    joyword_shell::formatting::Unit::Points => "磅",
                                    joyword_shell::formatting::Unit::Picas => "派卡",
                                }))
                            }
                            _ => editor,
                        };
                        body = body.child(
                            div()
                                .flex()
                                .items_center()
                                .gap_2()
                                .child(div().text_sm().w(px(165.)).flex_shrink_0().child(label))
                                .child(div().flex_1().min_w_0().child(editor)),
                        );
                    }
                    FieldState::Multiline(state) => {
                        body = body
                            .child(div().text_sm().child(label.clone()))
                            .child(Textarea::new(state).h(px(110.)).aria_label(label));
                    }
                    FieldState::Choice(state) => {
                        body = body.child(
                            div()
                                .flex()
                                .items_center()
                                .gap_2()
                                .child(
                                    div()
                                        .text_sm()
                                        .w(px(165.))
                                        .flex_shrink_0()
                                        .child(label.clone()),
                                )
                                .child(
                                    div().flex_1().min_w_0().child(
                                        gpui::component::select::Select::new(state)
                                            .accessibility_label(label)
                                            .placeholder("多种值 / 未设置")
                                            .small()
                                            .w_full(),
                                    ),
                                ),
                        );
                    }
                    FieldState::Check(state) => {
                        let value = *state.read(cx);
                        let state = state.clone();
                        body = body
                            .child(
                                gpui::component::checkbox::Checkbox::new(("form-checkbox", index))
                                    .label(label)
                                    .checked(value == Some(true))
                                    .on_change(move |checked, _, cx| {
                                        state.update(cx, |value, cx| {
                                            *value = Some(*checked);
                                            cx.notify();
                                        });
                                    }),
                            )
                            .when(value.is_none(), |body| {
                                body.child(div().text_xs().child("多种值，未更改时保留"))
                            });
                    }
                }
            }
            let preview = if command == "format.set" {
                let font = inputs
                    .first()
                    .map(|(field, state)| state.value(field, cx))
                    .unwrap_or_default();
                let size = inputs
                    .get(2)
                    .map(|(field, state)| state.value(field, cx))
                    .and_then(|s| s.parse::<f32>().ok())
                    .filter(|n| n.is_finite())
                    .unwrap_or(12.);
                Some(
                    div()
                        .border_1()
                        .border_color(rgb(0xcccccc))
                        .p_3()
                        .child(div().text_xs().child("字体与字号预览"))
                        .child(
                            div()
                                .font_family(SharedString::from(font))
                                .text_size(px(size.clamp(8., 36.)))
                                .child("中文字体 AaBbCc 123"),
                        ),
                )
            } else {
                None
            };
            let (view, close, cancel) = (view.clone(), view.clone(), view.clone());
            let inputs = inputs.clone();
            let target = target.clone();
            let error2 = error.clone();
            let parse = parse.clone();
            let apply = apply.clone();
            d.title(title)
                .width(px((f32::from(w.viewport_size().width) - 48.).min(520.)))
                .child(body)
                .when_some(preview, |dialog, preview| dialog.child(preview))
                .child(
                    div()
                        .text_color(rgb(0xc42b1c))
                        .child(error.read(cx).clone()),
                )
                .on_close(move |_, w, cx| {
                    let _ = close.update(cx, |v, cx| w.focus(&v.focus, cx));
                })
                .child(
                    div()
                        .flex()
                        .justify_end()
                        .gap_2()
                        .debug_selector(|| "authoring-actions".into())
                        .child(
                            Button::new("cancel-authoring")
                                .ghost()
                                .label("取消")
                                .on_click(move |_, w, cx| {
                                    w.close_dialog(cx);
                                    let _ = cancel.update(cx, |v, cx| w.focus(&v.focus, cx));
                                }),
                        )
                        .child(
                            Button::new("apply-authoring")
                                .primary()
                                .label("应用")
                                .on_click(move |_, w, cx| {
                                    let _ = view.update(cx, |v, cx| {
                                        if v.canvas.reader.as_ref().map(|r| {
                                            (
                                                r.engine().document_id(),
                                                r.engine().rev(),
                                                r.presentation_revision(),
                                                r.engine().sel.clone(),
                                            )
                                        }) != Some(target.clone())
                                        {
                                            error2.update(cx, |e, cx| {
                                                *e = "文档或选区已变化，请重新打开。".into();
                                                cx.notify();
                                            });
                                            return;
                                        }
                                        if inputs.iter().any(|(_, state)| match state {
                                            FieldState::Text(state) => state.update(cx, |s, cx| {
                                                s.marked_text_range(w, cx).is_some()
                                            }),
                                            FieldState::Multiline(state) => state
                                                .update(cx, |s, cx| {
                                                    s.marked_text_range(w, cx).is_some()
                                                }),
                                            _ => false,
                                        }) {
                                            return;
                                        }
                                        let result = inputs
                                            .iter()
                                            .map(|(field, state)| {
                                                field
                                                    .submitted(state.value(field, cx), changed_only)
                                            })
                                            .collect::<Result<Vec<_>, _>>()
                                            .and_then(|values| {
                                                if changed_only
                                                    && values.iter().all(String::is_empty)
                                                {
                                                    return Ok(serde_json::json!({}));
                                                }
                                                parse(&values)
                                                    .and_then(|args| apply(v, &args, w, cx))
                                            });
                                        match result {
                                            Ok(result) => {
                                                if command == "styles.create" {
                                                    v.style_pick =
                                                        result["id"].as_str().map(str::to_string);
                                                    v.styles_open = true;
                                                }
                                                w.close_dialog(cx);
                                                w.focus(&v.focus, cx);
                                            }
                                            Err(e) => {
                                                error2.update(cx, |v, cx| {
                                                    *v = e;
                                                    cx.notify();
                                                });
                                                if let Some((_, FieldState::Text(first))) =
                                                    inputs.first()
                                                {
                                                    w.focus(&first.focus_handle(cx), cx);
                                                }
                                            }
                                        }
                                    });
                                }),
                        ),
                )
        });
    }
}

#[cfg(test)]
mod field_tests {
    use super::*;
    use joyword_shell::formatting::Unit;
    #[test]
    fn rounded_units_preserve_unedited_precision_and_parse_explicit_units() {
        let field = Field::length("宽度", "451.28003", Unit::Centimeters);
        let shown = field.displayed();
        assert_eq!(field.submitted(shown.clone(), false).unwrap(), "451.28003");
        assert_eq!(field.submitted(shown, true).unwrap(), "");
        assert_eq!(
            field
                .submitted("2.54".into(), true)
                .unwrap()
                .parse::<f32>()
                .unwrap(),
            72.
        );
        assert_eq!(
            field
                .submitted("1 in".into(), true)
                .unwrap()
                .parse::<f32>()
                .unwrap(),
            72.
        );
        assert!(field.submitted("NaN".into(), true).is_err());
        assert!(field.submitted("1e39".into(), true).is_err());
    }
}
