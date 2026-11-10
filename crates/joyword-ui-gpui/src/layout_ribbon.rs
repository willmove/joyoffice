//! Layout ribbon over the shared editing session. Drafts and menus are selection-bound.
use super::ribbon::{Target, Tool};
use super::*;
use gpui::base::StepAction;
use gpui::component::{
    Icon,
    input::{Input, InputEvent, InputState},
    popover::Popover,
};
use joyoffice_ui::IconName;
use serde_json::{Value, json};

pub(super) struct LayoutControls {
    pub(super) inputs: [gpui::Entity<InputState>; 4],
    target: Option<Target>,
    values: [String; 4],
    basis: f64,
    unit: String,
    committing: bool,
    chinese: bool,
    section: Value,
}

#[cfg(test)]
#[allow(clippy::unwrap_used, reason = "GUI regression setup and assertions")]
mod tests {
    use super::*;
    use gpui::{Element, Focusable, RenderOnce, TestAppContext};
    use std::sync::atomic::AtomicBool;

    #[gpui::test]
    fn numeric_frames_do_not_claim_the_editor_accessibility_focus(cx: &mut TestAppContext) {
        cx.update(gpui_kit::init);
        let (view, cx) = cx.add_window_view(JoyWord::new);
        view.update_in(cx, |v, w, cx| {
            // TestPlatform does not activate AccessKit. Inspect the production
            // frame semantics explicitly; native prepaint is also verified on macOS.
            for index in 0..4 {
                let frame = layout_spin_frame(index, "Paragraph length", Some(12.), v.palette());
                let mut node = gpui::accesskit::Node::new(frame.a11y_role().unwrap());
                frame.write_a11y_info(&mut node);
                assert_eq!(node.role(), gpui::Role::SpinButton);
                assert_eq!(node.numeric_value(), Some(12.));
                assert!(!node.supports_action(gpui::AccessibleAction::Focus));
            }
            // The former composition really does advertise a second focus owner.
            // This is the precondition that the new non-focusing frame must avoid.
            let legacy = gpui::base::NumberInput::new(&v.layout.inputs[0])
                .input(Input::new(&v.layout.inputs[0]))
                .render(w, cx)
                .into_element();
            let mut legacy_node = gpui::accesskit::Node::new(gpui::Role::Unknown);
            legacy.write_a11y_info(&mut legacy_node);
            assert!(legacy_node.supports_action(gpui::AccessibleAction::Focus));
        });
    }

    #[gpui::test]
    fn every_numeric_editor_keeps_focus_through_steps_and_returns_it_on_enter(
        cx: &mut TestAppContext,
    ) {
        cx.update(gpui_kit::init);
        let (view, cx) = cx.add_window_view(JoyWord::new);
        view.update(cx, |v, cx| {
            let r = joyword_shell::reader::Reader::blank(&AtomicBool::new(false)).unwrap();
            let document = r.engine().document_id();
            v.canvas.reader = Some(r);
            v.install_input(document, cx);
            v.tab = 2;
        });
        cx.update(|w, cx| w.draw(cx).clear(cx));
        for (index, value) in ["1.27", "0.5", "12", "18"].into_iter().enumerate() {
            cx.update(|w, cx| {
                view.update(cx, |v, cx| {
                    v.layout.inputs[index].update(cx, |input, cx| {
                        input.set_value(value, w, cx);
                        input.focus(w, cx);
                    })
                });
                w.draw(cx).clear(cx);
            });
            cx.simulate_keystrokes("up");
            cx.update(|w, cx| {
                w.draw(cx).clear(cx);
                view.read_with(cx, |v, cx| {
                    assert!(v.layout.inputs[index].focus_handle(cx).is_focused(w));
                    assert_ne!(v.layout.inputs[index].read(cx).value().as_str(), value);
                });
            });
            cx.simulate_keystrokes("down enter");
            cx.update(|w, cx| {
                w.draw(cx).clear(cx);
                view.read_with(cx, |v, cx| {
                    assert!(v.focus.is_focused(w));
                    assert_eq!(v.layout.inputs[index].read(cx).value().as_str(), value);
                });
            });
        }
    }

    #[gpui::test]
    fn numeric_fields_commit_undo_preserve_mixed_values_and_reject_stale_drafts(
        cx: &mut TestAppContext,
    ) {
        cx.update(gpui_kit::init);
        let (view, cx) = cx.add_window_view(JoyWord::new);
        view.update(cx, |v, cx| {
            let mut r = joyword_shell::reader::Reader::blank(&AtomicBool::new(false)).unwrap();
            r.execute("document.setText", &json!({"text":"first\nsecond"}))
                .unwrap();
            r.execute("caret.docStart", &json!({})).unwrap();
            r.execute("para.spacing", &json!({"before":12,"after":18}))
                .unwrap();
            r.execute("caret.docEnd", &json!({})).unwrap();
            r.execute("para.spacing", &json!({"before":6,"after":24}))
                .unwrap();
            r.execute("select.all", &json!({})).unwrap();
            let document = r.engine().document_id();
            v.canvas.reader = Some(r);
            v.install_input(document, cx);
            v.tab = 2;
        });
        cx.update(|w, cx| w.draw(cx).clear(cx));
        view.update_in(cx, |v, w, cx| {
            assert!(v.layout.values[2].is_empty());
            assert!(v.layout.values[3].is_empty());
            v.layout.inputs[2].update(cx, |s, cx| s.set_value("30", w, cx));
            v.layout_input_event(
                2,
                &InputEvent::PressEnter {
                    secondary: false,
                    shift: false,
                },
                w,
                cx,
            );
            assert_eq!(
                v.canvas.reader.as_ref().unwrap().formatting().paragraph["spaceBefore"],
                30.
            );
            assert!(
                v.canvas.reader.as_ref().unwrap().formatting().paragraph["spaceAfter"].is_null()
            );
            assert_eq!(v.layout.values[2], "30");
            v.run_format("edit.undo", &json!({}), w, cx);
            v.sync_format(w, cx);
            v.sync_layout_controls(w, cx);
            assert!(v.layout.values[2].is_empty());
            // A navigation-only change invalidates an input even without a document revision.
            v.layout.inputs[2].update(cx, |s, cx| s.set_value("60", w, cx));
            v.canvas
                .reader
                .as_mut()
                .unwrap()
                .execute("caret.docStart", &json!({}))
                .unwrap();
            v.format_state = None;
            v.sync_format(w, cx);
            v.layout_input_event(2, &InputEvent::Blur, w, cx);
            assert_eq!(
                v.canvas.reader.as_ref().unwrap().formatting().paragraph["spaceBefore"],
                12.
            );
            // Spin controls use the same session and preserve the other spacing property.
            v.layout_step(2, StepAction::Increment, w, cx);
            assert_eq!(
                v.canvas.reader.as_ref().unwrap().formatting().paragraph["spaceBefore"],
                18.
            );
            assert_eq!(
                v.canvas.reader.as_ref().unwrap().formatting().paragraph["spaceAfter"],
                18.
            );
        });
    }

    #[test]
    fn numeric_units_and_ranges_do_not_silently_clamp_user_text() {
        assert_eq!(
            paragraph_parameter(0, "2.54", 72. / 2.54).unwrap()["left"],
            72.
        );
        assert_eq!(
            paragraph_parameter(1, "-1,27", 72. / 2.54).unwrap()["right"],
            -36.
        );
        assert!(paragraph_parameter(2, "-1", 1.).is_err());
        assert!(paragraph_parameter(2, "inf", 1.).is_err());
        assert!(paragraph_parameter(0, "10000", 72.).is_err());
    }
}

impl LayoutControls {
    pub(super) fn new(w: &mut Window, cx: &mut Context<JoyWord>) -> Self {
        Self {
            inputs: std::array::from_fn(|_| cx.new(|cx| InputState::new(w, cx))),
            target: None,
            values: Default::default(),
            basis: 1.,
            unit: String::new(),
            committing: false,
            chinese: true,
            section: Value::Null,
        }
    }
}

const PARA_KEYS: [&str; 4] = ["indentLeft", "indentRight", "spaceBefore", "spaceAfter"];

fn centimeters(points: f64) -> f64 {
    ((points * 2.54 / 72.) * 100. + 1e-9).round() / 100.
}

fn number(value: f64) -> String {
    format!("{value:.2}")
        .trim_end_matches('0')
        .trim_end_matches('.')
        .to_owned()
}

/// Accessibility container only: the nested Input owns the sole focus handle.
/// Tracking that handle here too aborts native debug builds during AccessKit prepaint.
fn layout_spin_frame(
    index: usize,
    label: &'static str,
    value: Option<f64>,
    palette: Palette,
) -> gpui::Stateful<gpui::Div> {
    div()
        .id(("layout-spin", index))
        .flex()
        .items_center()
        .w(px(76.))
        .h(px(21.))
        .rounded(px(2.))
        .border_1()
        .border_color(rgb(palette.border))
        .bg(rgb(palette.surface))
        .role(gpui::Role::SpinButton)
        .aria_label(label)
        .when_some(value, |element, value| element.aria_numeric_value(value))
}

fn paragraph_parameter(index: usize, text: &str, basis: f64) -> Result<Value, &'static str> {
    let value = text
        .trim()
        .replace(',', ".")
        .parse::<f64>()
        .map_err(|_| "请输入有效数值 / Enter a valid number")?;
    let points = value * if index < 2 { basis } else { 1. };
    let min = if index < 2 { -1584. } else { 0. };
    if !points.is_finite() || !(min..=1584.).contains(&points) {
        return Err("数值超出范围 / Value out of range");
    }
    let key = ["left", "right", "before", "after"][index];
    Ok(json!({key:points}))
}

#[derive(Clone)]
enum Action {
    Command(&'static str, Value),
    PageSetup,
    Columns,
    Paragraph,
    SelectionPane,
    Unavailable,
}
impl Action {
    fn command(&self) -> Option<&'static str> {
        match self {
            Self::Command(id, _) => Some(id),
            Self::PageSetup => Some("layout.pageSetup"),
            Self::Columns => Some("layout.columns"),
            Self::Paragraph => Some("para.set"),
            Self::SelectionPane => Some("arrange.selectionPane"),
            Self::Unavailable => None,
        }
    }
}

struct Entry {
    label: String,
    detail: String,
    icon: IconName,
    action: Action,
    selected: bool,
    heading: Option<&'static str>,
}

// Independently drawn geometry previews; no vendor assets are embedded.
fn entry_preview(entry: &Entry, palette: Palette) -> gpui::AnyElement {
    if let Action::Command("layout.margins", args) = &entry.action {
        let page = || {
            div()
                .relative()
                .w(px(32.))
                .h(px(42.))
                .border_1()
                .border_color(rgb(palette.disabled))
                .bg(rgb(palette.surface))
                .child(
                    div()
                        .absolute()
                        .top(px(args["top"].as_f64().unwrap_or(0.) as f32 / 792. * 42.))
                        .bottom(px(args["bottom"].as_f64().unwrap_or(0.) as f32 / 792. * 42.))
                        .left(px(args["left"].as_f64().unwrap_or(0.) as f32 / 612. * 32.))
                        .right(px(args["right"].as_f64().unwrap_or(0.) as f32 / 612. * 32.))
                        .border_1()
                        .border_color(rgb(palette.accent)),
                )
        };
        return div()
            .flex()
            .items_center()
            .justify_center()
            .w(px(40.))
            .flex_shrink_0()
            .child(page())
            .into_any_element();
    }
    Icon::new(entry.icon)
        .size(px(if entry.detail.is_empty() { 18. } else { 28. }))
        .into_any_element()
}

impl JoyWord {
    pub(super) fn sync_layout_controls(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        let target = self.ribbon_target();
        let basis = self
            .canvas
            .reader
            .as_ref()
            .map_or(1., |r| f64::from(r.engine().units().pt_per_unit()));
        let chinese = self.shell.locale == joyword_shell::l10n::Locale::ZhCn;
        if target == self.layout.target
            && basis == self.layout.basis
            && chinese == self.layout.chinese
        {
            return;
        }
        self.layout.target = target;
        self.layout.basis = basis;
        self.layout.chinese = chinese;
        self.layout.unit = self
            .canvas
            .reader
            .as_ref()
            .map_or(String::new(), |r| r.engine().units().suffix().trim().into());
        if self.shell.locale == joyword_shell::l10n::Locale::ZhCn {
            self.layout.unit = match self.layout.unit.as_str() {
                "cm" => "厘米",
                "mm" => "毫米",
                "pt" => "磅",
                "\"" => "英寸",
                _ => &self.layout.unit,
            }
            .into();
        }
        let para = self.format_state.as_ref().map(|s| &s.paragraph);
        for (index, key) in PARA_KEYS.iter().enumerate() {
            // Mixed selections and character-based indents stay blank until explicitly replaced.
            let value = para
                .and_then(|p| {
                    if index < 2
                        && (p[["indentLeftChars", "indentRightChars"][index]]
                            .as_f64()
                            .is_some_and(|v| v != 0.)
                            || p["mixed"].as_array().is_some_and(|keys| {
                                keys.iter().any(|v| {
                                    v.as_str()
                                        == Some(["indentLeftChars", "indentRightChars"][index])
                                })
                            }))
                    {
                        return None;
                    }
                    p[key].as_f64()
                })
                .map(|v| number(v / if index < 2 { basis } else { 1. }))
                .unwrap_or_default();
            self.layout.inputs[index].update(cx, |input, cx| input.set_value(value.clone(), w, cx));
            self.layout.values[index] = value;
        }
        self.layout.section = self
            .canvas
            .reader
            .as_mut()
            .and_then(|r| r.advanced_query("layout.section").ok())
            .unwrap_or(Value::Null);
    }

    pub(super) fn layout_input_event(
        &mut self,
        index: usize,
        event: &InputEvent,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.layout.committing {
            return;
        }
        if matches!(event, InputEvent::PressEnter { .. } | InputEvent::Blur) {
            self.commit_layout_input(index, w, cx);
            // Stepping has already committed the value. Enter must still leave
            // that editor, while an invalid draft keeps focus for correction.
            if matches!(event, InputEvent::PressEnter { .. })
                && self.layout.inputs[index].read(cx).value().as_str() == self.layout.values[index]
            {
                w.focus(&self.focus, cx);
            }
        }
    }

    fn commit_layout_input(&mut self, index: usize, w: &mut Window, cx: &mut Context<Self>) {
        let value = self.layout.inputs[index].read(cx).value().to_string();
        if value == self.layout.values[index] {
            return;
        }
        if self.ribbon_target() != self.layout.target || value.trim().is_empty() {
            self.layout.target = None;
            self.sync_layout_controls(w, cx);
            return;
        }
        let args = match paragraph_parameter(index, &value, self.layout.basis) {
            Ok(args) => args,
            Err(error) => {
                self.shell.notice = Some(error.into());
                cx.notify();
                return;
            }
        };
        self.layout.committing = true;
        let result = self.try_edit(
            if index < 2 {
                "para.indents"
            } else {
                "para.spacing"
            },
            &args,
            w,
            cx,
        );
        if result.is_ok() {
            self.layout.target = None;
            self.sync_format(w, cx);
            self.sync_layout_controls(w, cx);
        }
        self.layout.committing = false;
        if let Err(error) = result {
            self.shell.notice = Some(error);
            cx.notify();
        }
    }

    fn layout_step(
        &mut self,
        index: usize,
        action: StepAction,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.ribbon_target() != self.layout.target {
            self.sync_layout_controls(w, cx);
            return;
        }
        let current = self.layout.inputs[index].read(cx).value().parse::<f64>();
        let Ok(current) = current else {
            return;
        };
        let step = if index < 2 { 0.1 } else { 6. };
        let sign = if action == StepAction::Increment {
            1.
        } else {
            -1.
        };
        let min = if index < 2 {
            -1584. / self.layout.basis
        } else {
            0.
        };
        let max = 1584. / if index < 2 { self.layout.basis } else { 1. };
        let value = number((current + sign * step).clamp(min, max));
        self.layout.inputs[index].update(cx, |input, cx| input.set_value(value, w, cx));
        self.commit_layout_input(index, w, cx);
        self.layout.inputs[index].update(cx, |input, cx| input.focus(w, cx));
    }

    fn layout_number(&self, index: usize, cx: &mut Context<Self>) -> impl IntoElement {
        let p = self.palette();
        let (zh, en, icon) = [
            ("左:", "Left:", IconName::ListIndentIncrease),
            ("右:", "Right:", IconName::ListIndentDecrease),
            ("段前:", "Before:", IconName::ArrowUpToLine),
            ("段后:", "After:", IconName::ArrowDownToLine),
        ][index];
        let label = self.shell.tr(zh, en);
        let full_label = self.shell.tr(
            ["左缩进", "右缩进", "段前间距", "段后间距"][index],
            [
                "Left indent",
                "Right indent",
                "Spacing before",
                "Spacing after",
            ][index],
        );
        let enabled = self.format_ready(if index < 2 {
            "para.indents"
        } else {
            "para.spacing"
        });
        let weak = cx.entity().downgrade();
        let unit = if index < 2 {
            self.layout.unit.clone()
        } else {
            self.shell.tr("磅", "pt").into()
        };
        div()
            .id(("layout-number", index))
            .flex()
            .items_center()
            .gap(px(3.))
            .h(px(22.))
            .child(Icon::new(icon).size(px(16.)).text_color(rgb(p.disabled)))
            .child(
                div()
                    .w(px(if index < 2 {
                        if self.layout.chinese { 23. } else { 33. }
                    } else {
                        38.
                    }))
                    .text_size(px(11.))
                    .child(label),
            )
            .child(
                layout_spin_frame(
                    index,
                    full_label,
                    self.layout.inputs[index]
                        .read(cx)
                        .value()
                        .parse::<f64>()
                        .ok(),
                    p,
                )
                .child(
                    div().min_w_0().flex_1().child(
                        Input::new(&self.layout.inputs[index])
                            .xsmall()
                            .appearance(false)
                            .h(px(19.))
                            .px(px(3.))
                            .text_size(px(11.))
                            .disabled(!enabled)
                            .aria_label(full_label)
                            .suffix(
                                div()
                                    .text_size(px(10.))
                                    .text_color(rgb(p.disabled))
                                    .child(unit),
                            ),
                    ),
                )
                .child(
                    div().flex().flex_col().w(px(12.)).h_full().children(
                        [StepAction::Increment, StepAction::Decrement]
                            .into_iter()
                            .map(|action| {
                                let weak = weak.clone();
                                let increment = action == StepAction::Increment;
                                gpui::base::Button::new(if increment {
                                    "increment"
                                } else {
                                    "decrement"
                                })
                                .accessibility_label(format!(
                                    "{} {full_label}",
                                    if increment { "+" } else { "−" }
                                ))
                                .focusable(false)
                                .disabled(!enabled)
                                .flex()
                                .items_center()
                                .justify_center()
                                .w(px(12.))
                                .flex_1()
                                .min_h_0()
                                .p_0()
                                .hover(move |e| e.bg(rgb(p.hover)))
                                .child(
                                    Icon::new(if increment {
                                        IconName::ChevronUp
                                    } else {
                                        IconName::ChevronDown
                                    })
                                    .size(px(8.)),
                                )
                                .on_click(move |_, w, cx| {
                                    let _ = weak
                                        .update(cx, |v, cx| v.layout_step(index, action, w, cx));
                                })
                            }),
                    ),
                ),
            )
            .capture_key_down(cx.listener(move |v, e: &gpui::KeyDownEvent, w, cx| {
                if enabled && matches!(e.keystroke.key.as_str(), "up" | "down") {
                    v.layout_step(
                        index,
                        if e.keystroke.key == "up" {
                            StepAction::Increment
                        } else {
                            StepAction::Decrement
                        },
                        w,
                        cx,
                    );
                    cx.stop_propagation();
                } else if e.keystroke.key == "escape" {
                    v.layout.target = None;
                    v.sync_layout_controls(w, cx);
                    w.focus(&v.focus, cx);
                    cx.stop_propagation();
                }
            }))
    }

    fn layout_entry(
        &self,
        zh: &'static str,
        en: &'static str,
        icon: IconName,
        action: Action,
    ) -> Entry {
        Entry {
            label: self.shell.tr(zh, en).into(),
            detail: String::new(),
            icon,
            action,
            selected: false,
            heading: None,
        }
    }

    fn layout_menu(
        &self,
        id: &'static str,
        tool: Tool,
        entries: Vec<Entry>,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let weak = cx.entity().downgrade();
        let target = self.ribbon_target();
        let available: Vec<_> = entries
            .iter()
            .map(|e| e.action.command().is_some_and(|c| self.format_ready(c)))
            .collect();
        let enabled = available.iter().any(|v| *v)
            || (id == "layout-align" && self.format_ready("arrange.selectionPane"));
        let palette = self.palette();
        Popover::new(id).trigger(self.ribbon_tool(id, tool, enabled, true)).content(move |_,_,cx| {
            let popup = cx.entity().downgrade();
            let rows = entries.iter().zip(&available).enumerate().map(|(i,(entry,enabled))| {
                let (weak,popup,target,action) = (weak.clone(),popup.clone(),target.clone(),entry.action.clone());
                let row = Button::new((id,i+1)).ghost().small().w_full().h_auto().py(px(5.)).px(px(6.)).disabled(!enabled)
                    .accessibility_label(entry.label.clone()).selected(entry.selected)
                    .when(id == "layout-margins" && !entry.detail.is_empty(), |b| b.h(px(56.)))
                    .child(div().flex().items_center().w_full().gap(px(8.))
                        .child(entry_preview(entry,palette))
                        .child(div().flex().flex_col().items_start().gap(px(2.)).child(div().text_size(px(12.)).child(entry.label.clone()))
                            .when(!entry.detail.is_empty(), |e| e.child(div().text_size(px(10.)).whitespace_normal().child(entry.detail.clone())))))
                    .on_click(move |_,w,cx| {
                        let _ = popup.update(cx, |p,cx| p.dismiss(w,cx));
                        let _ = weak.update(cx, |v,cx| {
                            if v.ribbon_target() != target {
                                v.shell.notice = Some(v.shell.tr("文档或选区已变化，请重新打开菜单。", "The document or selection changed. Reopen the menu.").into()); cx.notify(); return;
                            }
                            v.perform_layout(action.clone(),w,cx);
                        });
                    });
                div().when_some(entry.heading, |e,h| e.child(div().text_size(px(11.)).font_weight(gpui::FontWeight::SEMIBOLD).px(px(6.)).py(px(4.)).child(h))).child(row)
            });
            div().id((id,0usize)).w(px(if id == "layout-margins" { 378. } else { 225. })).max_h(px(460.)).overflow_y_scroll().flex().flex_col().children(rows)
        })
    }

    fn perform_layout(&mut self, action: Action, w: &mut Window, cx: &mut Context<Self>) {
        let Some(command) = action.command() else {
            return;
        };
        if !self.format_ready(command) {
            return;
        }
        match action {
            Action::Command(id, args) => self.run_format(id, &args, w, cx),
            Action::PageSetup => self.open_page_setup(w, cx),
            Action::Columns => self.open_columns(w, cx),
            Action::Paragraph => self.format_dialog(formatting::Form::Paragraph, w, cx),
            Action::SelectionPane => {
                if self.advanced.pane == Some("对象") {
                    self.advanced.pane = None;
                    cx.notify();
                } else {
                    self.open_advanced("对象", cx);
                }
            }
            Action::Unavailable => {}
        }
    }

    pub(super) fn layout_ribbon(&self, cx: &mut Context<Self>) -> gpui::Div {
        use Action::{Command as C, Unavailable as U};
        use IconName as I;
        let entry = |zh, en, icon, action| self.layout_entry(zh, en, icon, action);
        let width = |zh, en| if self.layout.chinese { zh } else { en };
        let commands =
            |id, rows: &[(&'static str, &'static str, &'static str)], key: &str, icon| {
                rows.iter()
                    .map(|(zh, en, value)| entry(*zh, *en, icon, C(id, json!({key:*value}))))
                    .collect()
            };
        let mut margins: Vec<_> = [
            ("常规", "Normal", 72., 72., 90., 90., false),
            ("窄", "Narrow", 36., 36., 36., 36., false),
            ("中等", "Moderate", 72., 72., 54., 54., false),
            ("宽", "Wide", 72., 72., 144., 144., false),
            ("对称", "Mirrored", 72., 72., 90., 72., true),
        ]
        .into_iter()
        .map(|(zh, en, t, b, l, r, mirror)| {
            let mut e = entry(
                zh,
                en,
                I::PanelLeft,
                C(
                    "layout.margins",
                    json!({"top":t,"bottom":b,"left":l,"right":r,"mirror":mirror}),
                ),
            );
            e.detail = if self.shell.locale == joyword_shell::l10n::Locale::ZhCn {
                format!(
                    "顶部: {:.2} 厘米, 底部: {:.2} 厘米, 左侧: {:.2} 厘米, 右侧: {:.2} 厘米",
                    centimeters(t),
                    centimeters(b),
                    centimeters(l),
                    centimeters(r)
                )
            } else {
                format!(
                    "Top: {:.2} cm   Bottom: {:.2} cm\nLeft: {:.2} cm   Right: {:.2} cm",
                    centimeters(t),
                    centimeters(b),
                    centimeters(l),
                    centimeters(r)
                )
            };
            e.selected = self
                .canvas
                .reader
                .as_ref()
                .is_some_and(|r| r.engine().doc.settings.mirror_margins == mirror)
                && [
                    ("marginTop", t),
                    ("marginBottom", b),
                    ("marginLeft", l),
                    ("marginRight", r),
                ]
                .iter()
                .all(|(k, v)| {
                    self.layout.section[k]
                        .as_f64()
                        .is_some_and(|x| (x - v).abs() < 0.05)
                });
            e
        })
        .collect();
        margins.push(entry(
            "自定义边距…",
            "Custom Margins…",
            I::Settings,
            Action::PageSetup,
        ));
        let mut sizes: Vec<_> = [
            ("Letter", "21.59 × 27.94 cm"),
            ("Legal", "21.59 × 35.56 cm"),
            ("A3", "29.7 × 42 cm"),
            ("A4", "21 × 29.7 cm"),
            ("A5", "14.8 × 21 cm"),
            ("B5", "17.6 × 25 cm"),
        ]
        .into_iter()
        .map(|(name, size)| {
            let mut e = entry(name, name, I::File, C("layout.size", json!({"name":name})));
            e.detail = size.into();
            e
        })
        .collect();
        sizes.push(entry(
            "更多纸张大小…",
            "More Paper Sizes…",
            I::Settings,
            Action::PageSetup,
        ));
        let mut columns = Vec::new();
        for (zh, en, count) in [("一栏", "One", 1), ("两栏", "Two", 2), ("三栏", "Three", 3)]
        {
            columns.push(entry(
                zh,
                en,
                if count == 1 {
                    I::FileText
                } else if count == 2 {
                    I::Columns2
                } else {
                    I::Columns3
                },
                C("layout.columns", json!({"count":count,"space":36.})),
            ));
        }
        columns.push(entry(
            "偏左",
            "Left",
            I::Columns2,
            C("layout.columns", json!({"preset":"left"})),
        ));
        columns.push(entry(
            "偏右",
            "Right",
            I::Columns2,
            C("layout.columns", json!({"preset":"right"})),
        ));
        columns.push(entry(
            "更多分栏…",
            "More Columns…",
            I::Settings,
            Action::Columns,
        ));
        let mut breaks: Vec<Entry> = commands(
            "layout.break",
            &[
                ("页面", "Page", "page"),
                ("栏", "Column", "column"),
                ("下一页", "Next Page", "nextPage"),
                ("连续", "Continuous", "continuous"),
                ("偶数页", "Even Page", "evenPage"),
                ("奇数页", "Odd Page", "oddPage"),
            ],
            "kind",
            I::FileSymlink,
        );
        breaks[0].heading = Some(self.shell.tr("分页符", "Page Breaks"));
        breaks[2].heading = Some(self.shell.tr("分节符", "Section Breaks"));
        let page = div()
            .flex()
            .items_center()
            .gap_1()
            .child(self.layout_menu(
                "layout-direction",
                Tool::new("文本方向", "Text Direction", I::TextCursorInput).width(width(56., 68.)),
                commands(
                    "layout.textDirection",
                    &[
                        ("水平", "Horizontal", "horizontal"),
                        ("将所有文字旋转 90°", "Rotate all text 90°", "down"),
                        ("将所有文字旋转 270°", "Rotate all text 270°", "up"),
                    ],
                    "value",
                    I::TextCursorInput,
                ),
                cx,
            ))
            .child(self.layout_menu(
                "layout-margins",
                Tool::new("页边距", "Margins", I::PanelLeft),
                margins,
                cx,
            ))
            .child(self.layout_menu(
                "layout-orientation",
                Tool::new("方向", "Orientation", I::RectangleHorizontal).width(width(56., 72.)),
                commands(
                    "layout.orientation",
                    &[
                        ("纵向", "Portrait", "portrait"),
                        ("横向", "Landscape", "landscape"),
                    ],
                    "value",
                    I::File,
                ),
                cx,
            ))
            .child(self.layout_menu("layout-size", Tool::new("大小", "Size", I::File), sizes, cx))
            .child(self.layout_menu(
                "layout-columns",
                Tool::new("栏", "Columns", I::Columns2),
                columns,
                cx,
            ))
            .child(self.layout_menu(
                "layout-breaks",
                Tool::new("分隔符", "Breaks", I::FileSymlink),
                breaks,
                cx,
            ))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(
                        self.layout_menu(
                            "layout-lines",
                            Tool::compact("行号", "Line Numbers", I::ListOrdered)
                                .width(width(76., 120.)),
                            commands(
                                "layout.lineNumbers",
                                &[
                                    ("无", "None", "none"),
                                    ("连续", "Continuous", "continuous"),
                                    ("每页重编行号", "Restart Each Page", "restartPage"),
                                    ("每节重编行号", "Restart Each Section", "restartSection"),
                                ],
                                "value",
                                I::ListOrdered,
                            ),
                            cx,
                        ),
                    )
                    .child(self.layout_menu(
                        "layout-hyphenation",
                        Tool::compact("断字", "Hyphenation", I::TextWrap).width(width(76., 120.)),
                        vec![
                            entry(
                                "无",
                                "None",
                                I::Minus,
                                C("layout.hyphenation", json!({"value":false})),
                            ),
                            entry(
                                "自动",
                                "Automatic",
                                I::TextWrap,
                                C("layout.hyphenation", json!({"value":true})),
                            ),
                            entry("手动…", "Manual…", I::TextCursorInput, U),
                        ],
                        cx,
                    )),
            );
        let para = div()
            .flex()
            .items_center()
            .gap_2()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(1.))
                    .child(
                        div()
                            .text_size(px(11.))
                            .child(self.shell.tr("缩进", "Indent")),
                    )
                    .child(self.layout_number(0, cx))
                    .child(self.layout_number(1, cx)),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(1.))
                    .child(
                        div()
                            .text_size(px(11.))
                            .child(self.shell.tr("间距", "Spacing")),
                    )
                    .child(self.layout_number(2, cx))
                    .child(self.layout_number(3, cx)),
            )
            .child(
                Button::new("layout-paragraph-dialog")
                    .ghost()
                    .xsmall()
                    .p_0()
                    .w(px(18.))
                    .h(px(21.))
                    .icon(I::ArrowDownRight)
                    .accessibility_label(self.shell.tr("段落设置…", "Paragraph Settings…"))
                    .disabled(!self.format_ready("para.set"))
                    .on_click(
                        cx.listener(|v, _, w, cx| v.perform_layout(Action::Paragraph, w, cx)),
                    ),
            );
        let tool = |zh, en, icon| Tool::new(zh, en, icon).width(width(56., 64.));
        let positions = commands(
            "arrange.position",
            &[
                ("左上", "Top Left", "topLeft"),
                ("中上", "Top Center", "topCenter"),
                ("右上", "Top Right", "topRight"),
                ("左中", "Middle Left", "middleLeft"),
                ("正中", "Middle Center", "middleCenter"),
                ("右中", "Middle Right", "middleRight"),
                ("左下", "Bottom Left", "bottomLeft"),
                ("中下", "Bottom Center", "bottomCenter"),
                ("右下", "Bottom Right", "bottomRight"),
            ],
            "preset",
            I::PanelsTopLeft,
        );
        let wrap = commands(
            "arrange.wrap",
            &[
                ("嵌入型", "In Line with Text", "inline"),
                ("四周型", "Square", "square"),
                ("紧密型", "Tight", "tight"),
                ("穿越型", "Through", "through"),
                ("上下型", "Top and Bottom", "topAndBottom"),
                ("衬于文字下方", "Behind Text", "behindText"),
                ("浮于文字上方", "In Front of Text", "inFrontOfText"),
            ],
            "wrap",
            I::TextWrap,
        );
        let arrange = div()
            .flex()
            .items_center()
            .gap_1()
            .child(self.layout_menu(
                "layout-position",
                tool("位置", "Position", I::PanelsTopLeft),
                positions,
                cx,
            ))
            .child(self.layout_menu(
                "layout-wrap",
                tool("自动换行", "Wrap Text", I::TextWrap),
                wrap,
                cx,
            ))
            // Existing bringForward/sendBackward change text wrapping, not object z-order.
            .child(self.ribbon_tool(
                "layout-forward",
                tool("前移一层", "Bring Forward", I::BringToFront),
                false,
                true,
            ))
            .child(self.ribbon_tool(
                "layout-backward",
                tool("后移一层", "Send Backward", I::SendToBack),
                false,
                true,
            ))
            .child(
                self.ribbon_tool(
                    "layout-selection",
                    tool("选择窗格", "Selection Pane", I::PanelsLeftBottom),
                    self.format_ready("arrange.selectionPane"),
                    false,
                )
                .selected(self.advanced.pane == Some("对象"))
                .on_click(
                    cx.listener(|v, _, w, cx| v.perform_layout(Action::SelectionPane, w, cx)),
                ),
            )
            .child(self.layout_menu(
                "layout-align",
                tool("对齐", "Align", I::AlignStartVertical),
                commands(
                    "arrange.align",
                    &[
                        ("左对齐", "Align Left", "left"),
                        ("水平居中", "Align Center", "center"),
                        ("右对齐", "Align Right", "right"),
                    ],
                    "value",
                    I::AlignStartVertical,
                ),
                cx,
            ))
            .child(self.layout_menu(
                "layout-group",
                tool("组合", "Group", I::Group),
                vec![
                    entry("组合", "Group", I::Group, C("arrange.group", json!({}))),
                    entry(
                        "取消组合",
                        "Ungroup",
                        I::Ungroup,
                        C("arrange.ungroup", json!({})),
                    ),
                ],
                cx,
            ))
            .child(self.layout_menu(
                "layout-rotate",
                tool("旋转", "Rotate", I::RotateCw),
                commands(
                    "arrange.rotate",
                    &[
                        ("向右旋转 90°", "Rotate Right 90°", "right"),
                        ("向左旋转 90°", "Rotate Left 90°", "left"),
                        ("垂直翻转", "Flip Vertical", "flipVertical"),
                        ("水平翻转", "Flip Horizontal", "flipHorizontal"),
                    ],
                    "direction",
                    I::RotateCw,
                ),
                cx,
            ));
        let tr = |zh, en| self.shell.tr(zh, en);
        div()
            .flex()
            .items_center()
            .flex_shrink_0()
            .child(self.ribbon_group("layout-page-setup", tr("页面设置", "Page Setup"), page))
            .child(self.ribbon_group("layout-paragraph", tr("段落", "Paragraph"), para))
            .child(self.ribbon_group("layout-arrange", tr("排列", "Arrange"), arrange))
    }
}
