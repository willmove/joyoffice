//! Font, colour and border controls independently arranged from local Excel observations.
use crate::desktop::Desktop;
use gpui_kit::{
    component::{
        Sizable,
        button::{Button, ButtonVariants},
        input::Input,
        popover::Popover,
    },
    prelude::*,
    *,
};
use joysheet_engine::model::{Color, Theme};
use serde_json::{Value, json};
use std::rc::Rc;

pub(crate) type ColorPick = Rc<dyn Fn(Color, &mut Window, &mut App)>;
pub(crate) fn color_parameter(color: Color) -> Value {
    match color {
        Color::Auto => json!("auto"),
        Color::Rgb(v) => json!(format!("#{v:06X}")),
        Color::Theme(i, t) => json!({"theme":i,"tint":f64::from(t)/1000.}),
    }
}
pub(crate) fn theme_swatches(theme: &Theme) -> Vec<(Color, u32)> {
    let mut colors = Vec::new();
    for row in 0..6 {
        for i in 0..10 {
            let tint = if row == 0 {
                0
            } else if i == 0 {
                [0, -50, -150, -250, -350, -500][row]
            } else if i == 1 {
                [0, 500, 350, 250, 150, 50][row]
            } else {
                [0, 800, 600, 400, -250, -500][row]
            };
            let color = Color::Theme(i, tint);
            let rgb = color.resolve(theme).map(|[r, g, b]| (u32::from(r) << 16) | (u32::from(g) << 8) | u32::from(b)).unwrap_or(0);
            colors.push((color, rgb));
        }
    }
    colors
}
pub(crate) fn swatch_grid(id: &str, colors: &[(Color, u32)], columns: usize, pick: ColorPick, owner: WeakEntity<Desktop>) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .gap(px(2.))
        .children(colors.chunks(columns).enumerate().map(|(row, colors)| {
            let pick = pick.clone();
            let id = id.to_string();
            let owner = owner.clone();
            div().flex().gap(px(2.)).children(colors.iter().enumerate().map(move |(column, &(color, rgb_value))| {
                let pick = pick.clone();
                Button::new(SharedString::from(format!("{id}-{row}-{column}")))
                    .relative()
                    .child(crate::excel_workflows::bound_probe(owner.clone(), format!("{id}:{row}:{column}")))
                    .ghost()
                    .xsmall()
                    .size(px(21.))
                    .p_0()
                    .accessibility_label(format!("#{rgb_value:06X}"))
                    .child(div().size(px(18.)).bg(rgb(rgb_value)).border_1().border_color(rgb(0xb0b0b0)))
                    .on_click(move |_, w, cx| pick(color, w, cx))
            }))
        }))
        .into_any_element()
}
pub(crate) const STANDARD_COLORS: [u32; 10] = [0xc00000, 0xff0000, 0xffc000, 0xffff00, 0x92d050, 0x00b050, 0x00b0f0, 0x0070c0, 0x002060, 0x7030a0];

impl Desktop {
    pub(crate) fn color_picker_field(&self, key: &str, cx: &mut Context<Self>) -> AnyElement {
        let key = key.to_string();
        let value = self.dialog_text(&key, cx);
        let shown = u32::from_str_radix(value.trim_start_matches('#'), 16).unwrap_or(0);
        let theme = self.view().map(|v| v.workbook.theme.clone()).unwrap_or_default();
        let owner = cx.weak_entity();
        let chinese = self.chinese();
        let label = crate::palette::field_label(&key, chinese);
        let field = self.command_fields.iter().find(|f| f.key == key).map(|f| f.value.clone());
        let watcher = owner.clone();
        let picker = Popover::new(SharedString::from(format!("colour-picker-{key}")))
            .on_open_change(move |open, _, cx| {
                let _ = watcher.update(cx, |v, _| v.excel_ui.popup_open = *open);
            })
            .trigger(
                Button::new(SharedString::from(format!("colour-trigger-{key}")))
                    .label("▾")
                    .accessibility_label(label.clone())
                    .child(div().w(px(30.)).h(px(16.)).bg(rgb(shown))),
            )
            .content(move |_, _, cx| {
                let popup = cx.entity().downgrade();
                let owner = owner.clone();
                let bounds_owner = owner.clone();
                let key = key.clone();
                let pick: ColorPick = Rc::new(move |color, w, cx| {
                    let _ = popup.update(cx, |p, cx| p.dismiss(w, cx));
                    let _ = owner.update(cx, |v, cx| v.set_dialog_color(&key, color, w, cx));
                });
                div()
                    .p_2()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(if chinese { "主题颜色" } else { "Theme Colors" })
                    .child(swatch_grid("theme-colour", &theme_swatches(&theme), 10, pick.clone(), bounds_owner.clone()))
                    .child(if chinese { "标准色" } else { "Standard Colors" })
                    .child(swatch_grid("standard-colour", &STANDARD_COLORS.map(|v| (Color::Rgb(v), v)), 10, pick, bounds_owner))
            });
        div()
            .flex()
            .items_center()
            .gap_2()
            .child(div().w(px(120.)).child(label))
            .children(field.map(|f| Input::new(&f).flex_1().into_any_element()))
            .child(picker)
            .into_any_element()
    }
    pub(crate) fn font_dialog_body(&self, id: &str, cx: &mut Context<Self>) -> AnyElement {
        let key = if id == "home.fontName" { "name" } else { "fontName" };
        let selected = self.dialog_text(key, cx);
        let family = selected.clone();
        let size = self.dialog_text("fontSize", cx).parse::<f32>().unwrap_or(11.);
        let mut names = self.excel_ui.fonts.clone();
        names.sort_by_key(|name| (name != &selected, name.to_lowercase()));
        let mut left = div().flex().flex_col().gap_2().flex_1().child(self.label("字体", "Font"));
        if let Some(field) = self.command_fields.iter().find(|f| f.key == key) {
            left = left.child(Input::new(&field.value).aria_label(self.label("字体", "Font")));
        }
        left = left.child(
            div()
                .id("font-family-list")
                .h(px(170.))
                .overflow_y_scroll()
                .border_1()
                .border_color(rgb(self.palette().border))
                .flex()
                .flex_col()
                .children(names.into_iter().take(1024).enumerate().map(|(i, name)| {
                    let key = key.to_string();
                    let label = name.clone();
                    Button::new(("font-family", i))
                        .ghost()
                        .xsmall()
                        .justify_start()
                        .label(label)
                        .on_click(cx.listener(move |v, _, w, cx| v.set_dialog_value(&key, json!(name), w, cx)))
                })),
        );
        let mut body = div().flex().flex_col().gap_3().p_4();
        if id == "home.fontName" {
            return body.child(left).into_any_element();
        }
        let mut style = div().w(px(160.)).flex().flex_col().gap_2().child(self.label("字形", "Font style"));
        for (zh, en, bold, italic) in [
            ("常规", "Regular", false, false),
            ("倾斜", "Italic", false, true),
            ("加粗", "Bold", true, false),
            ("加粗倾斜", "Bold Italic", true, true),
        ] {
            style = style.child(
                Button::new(en)
                    .relative()
                    .child(self.advanced_probe(format!("font-style:{en}"), cx))
                    .ghost()
                    .justify_start()
                    .label(self.label(zh, en))
                    .on_click(cx.listener(move |v, _, w, cx| {
                        v.set_dialog_value("bold", json!(bold), w, cx);
                        v.set_dialog_value("italic", json!(italic), w, cx);
                    })),
            );
        }
        body = body.child(div().flex().gap_3().child(left).child(style));
        for key in ["fontSize", "underline", "strike", "fontScript"] {
            if let Some(f) = self.command_fields.iter().find(|f| f.key == key) {
                body = body.child(self.dialog_field(id, f, cx));
            }
        }
        body = body.child(self.color_picker_field("fontColor", cx));
        let ink = u32::from_str_radix(self.dialog_text("fontColor", cx).trim_start_matches('#'), 16).unwrap_or(0);
        body.child(div().text_sm().child(self.label("预览", "Preview")))
            .child(
                div()
                    .h(px(75.))
                    .border_1()
                    .border_color(rgb(self.palette().border))
                    .flex()
                    .items_center()
                    .justify_center()
                    .bg(rgb(self.palette().surface))
                    .font_family(SharedString::from(family))
                    .text_size(px(size.clamp(8., 36.)))
                    .text_color(rgb(ink))
                    .when(self.dialog_text("bold", cx) == "true", |el| el.font_weight(FontWeight::BOLD))
                    .when(self.dialog_text("italic", cx) == "true", |el| el.italic())
                    .child("AaBbYyZz　中文"),
            )
            .into_any_element()
    }
    pub(crate) fn border_dialog_body(&self, cx: &mut Context<Self>) -> AnyElement {
        let mut body = div().p_4().flex().flex_col().gap_3();
        for key in ["borderStyle"] {
            if let Some(f) = self.command_fields.iter().find(|f| f.key == key) {
                body = body.child(self.dialog_field("home.formatCells", f, cx));
            }
        }
        body = body.child(self.color_picker_field("borderColor", cx));
        let mut presets = div().flex().gap_2().child(self.label("预置", "Presets"));
        for (preset, zh, en) in [("none", "无", "None"), ("outside", "外边框", "Outline"), ("inside", "内部", "Inside")] {
            presets = presets.child(
                Button::new(SharedString::from(format!("border-preset-{preset}")))
                    .relative()
                    .child(self.advanced_probe(format!("border-preset:{preset}"), cx))
                    .label(self.label(zh, en))
                    .on_click(cx.listener(move |v, _, w, cx| v.border_preset(preset, w, cx))),
            );
        }
        let sides = ["borderLeft", "borderRight", "borderTop", "borderBottom", "borderDown", "borderUp", "borderInnerH", "borderInnerV"];
        let lines = sides.map(|key| {
            let style = self.dialog_text(key, cx);
            let col = u32::from_str_radix(self.dialog_text(&format!("{key}Color"), cx).trim_start_matches('#'), 16).unwrap_or(0);
            (style, col)
        });
        let mut preview = div().relative().w(px(300.)).h(px(180.)).bg(rgb(0xffffff)).child(
            canvas(
                |_, _, _| {},
                move |bounds, _, window, _| {
                    let x = f32::from(bounds.origin.x);
                    let y = f32::from(bounds.origin.y);
                    let paths: [(f32, f32, f32, f32); 8] = [
                        (35., 25., 35., 155.),
                        (265., 25., 265., 155.),
                        (35., 25., 265., 25.),
                        (35., 155., 265., 155.),
                        (35., 25., 265., 155.),
                        (35., 155., 265., 25.),
                        (35., 90., 265., 90.),
                        (150., 25., 150., 155.),
                    ];
                    for ((style, col), &(x1, y1, x2, y2)) in lines.iter().zip(paths.iter()) {
                        if style == "None" {
                            continue;
                        }
                        let n = ((x2 - x1).abs().max((y2 - y1).abs()) as usize).max(1);
                        for i in 0..=n {
                            if matches!(style.as_str(), "Dashed" | "Dotted") && i % 8 > 3 {
                                continue;
                            }
                            let t = i as f32 / n as f32;
                            let w = if matches!(style.as_str(), "Thick" | "Medium") { 2. } else { 1. };
                            window.paint_quad(fill(
                                Bounds::new(point(px(x + x1 + (x2 - x1) * t), px(y + y1 + (y2 - y1) * t)), size(px(w), px(w))),
                                rgb(*col),
                            ));
                            if style == "Double" {
                                window.paint_quad(fill(
                                    Bounds::new(point(px(x + x1 + (x2 - x1) * t + 3.), px(y + y1 + (y2 - y1) * t + 3.)), size(px(1.), px(1.))),
                                    rgb(*col),
                                ));
                            }
                        }
                    }
                },
            )
            .w_full()
            .h_full(),
        );
        for (key, label, x, y, w, h) in [
            ("borderTop", "上", 35., 5., 230., 20.),
            ("borderBottom", "下", 35., 155., 230., 20.),
            ("borderLeft", "左", 5., 30., 30., 120.),
            ("borderRight", "右", 265., 30., 30., 120.),
            ("borderDown", "↘", 90., 70., 45., 35.),
            ("borderUp", "↗", 165., 70., 45., 35.),
            ("borderInnerH", "—", 120., 75., 60., 25.),
            ("borderInnerV", "│", 140., 105., 25., 30.),
        ] {
            preview = preview.child(
                div()
                    .id(key)
                    .role(Role::Button)
                    .tab_stop(true)
                    .cursor_pointer()
                    .flex()
                    .items_center()
                    .justify_center()
                    .absolute()
                    .left(px(x))
                    .top(px(y))
                    .w(px(w))
                    .h(px(h))
                    .child(label)
                    .aria_label(match key {
                        "borderTop" => self.label("上边框", "Top border"),
                        "borderBottom" => self.label("下边框", "Bottom border"),
                        "borderLeft" => self.label("左边框", "Left border"),
                        "borderRight" => self.label("右边框", "Right border"),
                        "borderDown" => self.label("斜下边框", "Diagonal down border"),
                        "borderUp" => self.label("斜上边框", "Diagonal up border"),
                        "borderInnerH" => self.label("内部横边框", "Inside horizontal border"),
                        _ => self.label("内部竖边框", "Inside vertical border"),
                    })
                    .child(self.advanced_probe(format!("border-preview:{key}"), cx))
                    .on_click(cx.listener(move |v, _, w, cx| v.toggle_border(key, w, cx)))
                    .on_key_down(cx.listener(move |v, e: &KeyDownEvent, w, cx| {
                        if matches!(e.keystroke.key.as_str(), "enter" | "space") {
                            v.toggle_border(key, w, cx);
                            cx.stop_propagation();
                        }
                    })),
            );
        }
        body.child(presets)
            .child(div().flex().justify_center().child(preview))
            .child(self.label("单击预览边缘设置或取消边框。", "Click preview edges to add or remove borders."))
            .into_any_element()
    }
    pub(crate) fn more_colors_body(&self, cx: &mut Context<Self>) -> AnyElement {
        let mut body = div().p_4().flex().flex_col().gap_3();
        if self.dialog_tab == 0 {
            let colors = (0..10)
                .flat_map(|r| {
                    (0..12).map(move |h| {
                        let (v, s) = (1. - r as f32 * 0.085, 0.15 + r as f32 * 0.085);
                        let rgb = hsv_rgb(h as f32 / 12., s, v);
                        (Color::Rgb(rgb), rgb)
                    })
                })
                .collect::<Vec<_>>();
            let owner = cx.weak_entity();
            let pick: ColorPick = Rc::new(move |c, w, cx| {
                let _ = owner.update(cx, |v, cx| v.set_dialog_color("color", c, w, cx));
            });
            body = body.child(swatch_grid("colour-standard", &colors, 12, pick, cx.weak_entity()));
        } else {
            for key in ["red", "green", "blue", "color"] {
                if let Some(f) = self.command_fields.iter().find(|f| f.key == key) {
                    body = body.child(self.dialog_field("colour", f, cx));
                }
            }
        }
        let current =
            self.command_initial["color"].as_str().and_then(|s| u32::from_str_radix(s.trim_start_matches('#'), 16).ok()).unwrap_or(0xffffff);
        let next = self.proposed_rgb(cx).unwrap_or(current);
        body.child(
            div()
                .flex()
                .gap_3()
                .child(
                    div()
                        .flex_1()
                        .flex()
                        .flex_col()
                        .child(self.label("新增", "New"))
                        .child(div().h(px(45.)).bg(rgb(next)).border_1().border_color(rgb(self.palette().border))),
                )
                .child(
                    div()
                        .flex_1()
                        .flex()
                        .flex_col()
                        .child(self.label("当前", "Current"))
                        .child(div().h(px(45.)).bg(rgb(current)).border_1().border_color(rgb(self.palette().border))),
                ),
        )
        .into_any_element()
    }
}
fn hsv_rgb(h: f32, s: f32, v: f32) -> u32 {
    let h = h * 6.;
    let c = v * s;
    let x = c * (1. - (h % 2. - 1.).abs());
    let (r, g, b) = match h as u8 {
        0 => (c, x, 0.),
        1 => (x, c, 0.),
        2 => (0., c, x),
        3 => (0., x, c),
        4 => (x, 0., c),
        _ => (c, 0., x),
    };
    let m = v - c;
    ((255. * (r + m)).round() as u32) << 16 | ((255. * (g + m)).round() as u32) << 8 | (255. * (b + m)).round() as u32
}
