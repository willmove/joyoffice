//! Visual selectors use the same supported styles and types as the engine.
use crate::{
    desktop::Desktop,
    excel_controls::{ColorPick, STANDARD_COLORS, swatch_grid, theme_swatches},
};
use gpui_kit::{
    component::button::{Button, ButtonVariants},
    prelude::*,
    *,
};
use joysheet_engine::model::Color;
use serde_json::json;
use std::rc::Rc;
pub(crate) type GalleryPick = Rc<dyn Fn(String, &mut Window, &mut App)>;
impl Desktop {
    pub(crate) fn table_gallery(&self, pick: GalleryPick, cx: &Context<Self>) -> AnyElement {
        let theme = self.view().map(|v| v.workbook.theme.clone()).unwrap_or_default();
        let mut body = div().id("table-gallery").w(px(500.)).max_h(px(580.)).overflow_y_scroll().p_2().flex().flex_col().gap_2();
        for (family, zh, en) in [("Light", "浅色", "Light"), ("Medium", "中等色", "Medium"), ("Dark", "深色", "Dark")] {
            body = body.child(self.label(zh, en));
            let styles = joysheet_engine::tables::gallery()
                .into_iter()
                .filter(|s| joysheet_engine::tables::parse_style_name(s).0 == family)
                .collect::<Vec<_>>();
            for row in styles.chunks(7) {
                let mut line = div().flex().gap_1();
                for style in row {
                    let look = joysheet_engine::tables::look(style);
                    let color = |c: Option<Color>, fallback: u32| {
                        c.and_then(|c| c.resolve(&theme)).map(|[r, g, b]| u32::from(r) << 16 | u32::from(g) << 8 | u32::from(b)).unwrap_or(fallback)
                    };
                    let header = color(look.header_fill, 0xffffff);
                    let band = color(look.band_fill, 0xf0f0f0);
                    let base = color(look.body_fill, 0xffffff);
                    let border = color(look.line, 0xd0d0d0);
                    let name = style.clone();
                    let pick = pick.clone();
                    let thumb = div().w(px(60.)).h(px(44.)).flex().flex_col().children((0..5).map(|r| {
                        div()
                            .flex_1()
                            .bg(rgb(if r == 0 {
                                header
                            } else if r % 2 == 0 {
                                band
                            } else {
                                base
                            }))
                            .flex()
                            .children((0..4).map(|_| {
                                div()
                                    .flex_1()
                                    .border_b_1()
                                    .border_r_1()
                                    .border_color(rgb(border))
                                    .m(px(1.))
                                    .child(div().mt(px(3.)).mx_1().h(px(1.)).bg(rgb(0x777777)))
                            }))
                    }));
                    line = line.child(
                        Button::new(SharedString::from(style.clone()))
                            .ghost()
                            .p_1()
                            .h(px(52.))
                            .accessibility_label(style.clone())
                            .child(div().relative().child(self.advanced_probe(format!("table-style:{style}"), cx)).child(thumb))
                            .on_click(move |_, w, cx| pick(name.clone(), w, cx)),
                    );
                }
                body = body.child(line);
            }
        }
        body.into_any_element()
    }
    pub(crate) fn ribbon_colour_palette(
        &self,
        command: &str,
        pick: ColorPick,
        more: Rc<dyn Fn(&mut Window, &mut App)>,
        cx: &Context<Self>,
    ) -> AnyElement {
        let theme = self.view().map(|v| v.workbook.theme.clone()).unwrap_or_default();
        let auto = pick.clone();
        let mut body = div()
            .p_2()
            .w(px(254.))
            .flex()
            .flex_col()
            .gap_2()
            .child(self.label("主题颜色", "Theme Colors"))
            .child(swatch_grid("ribbon-theme", &theme_swatches(&theme), 10, pick.clone(), cx.weak_entity()))
            .child(self.label("标准色", "Standard Colors"))
            .child(swatch_grid("ribbon-standard", &STANDARD_COLORS.map(|c| (Color::Rgb(c), c)), 10, pick, cx.weak_entity()));
        body = body.child(
            Button::new("colour-auto")
                .ghost()
                .justify_start()
                .label(if command == "fill" { self.label("无填充", "No Fill") } else { self.label("自动", "Automatic") })
                .on_click(move |_, w, cx| auto(Color::Auto, w, cx)),
        );
        body.child(
            Button::new("colour-more").ghost().justify_start().label(self.label("其他颜色…", "More Colors…")).on_click(move |_, w, cx| more(w, cx)),
        )
        .into_any_element()
    }
    pub(crate) fn visual_gallery_body(&self, id: &str, cx: &mut Context<Self>) -> Option<AnyElement> {
        let (key, items): (&str, Vec<(&str, &str, joyoffice_ui::IconName)>) = match id {
            "insert.chart" | "chart.changeType" => (
                "type",
                vec![
                    ("column", "柱形图", joyoffice_ui::IconName::ChartColumn),
                    ("bar", "条形图", joyoffice_ui::IconName::ChartBar),
                    ("line", "折线图", joyoffice_ui::IconName::ChartLine),
                    ("pie", "饼图", joyoffice_ui::IconName::ChartPie),
                    ("scatter", "散点图", joyoffice_ui::IconName::ChartNoAxesCombined),
                    ("area", "面积图", joyoffice_ui::IconName::ChartLine),
                ],
            ),
            "insert.shape" => (
                "kind",
                vec![
                    ("rectangle", "矩形", joyoffice_ui::IconName::Square),
                    ("ellipse", "椭圆", joyoffice_ui::IconName::Circle),
                    ("line", "直线", joyoffice_ui::IconName::Minus),
                    ("arrow", "箭头", joyoffice_ui::IconName::ArrowRight),
                    ("triangle", "三角形", joyoffice_ui::IconName::Triangle),
                    ("roundedRectangle", "圆角矩形", joyoffice_ui::IconName::Square),
                    ("textBox", "文本框", joyoffice_ui::IconName::Type),
                ],
            ),
            "insert.smartArt" => (
                "layout",
                vec![
                    ("process", "流程", joyoffice_ui::IconName::ArrowRight),
                    ("list", "列表", joyoffice_ui::IconName::List),
                    ("hierarchy", "层次结构", joyoffice_ui::IconName::Shapes),
                    ("cycle", "循环", joyoffice_ui::IconName::RotateCcw),
                ],
            ),
            _ => return None,
        };
        let selected = self.dialog_text(key, cx);
        let mut body = div().p_4().flex().flex_col().gap_3();
        let mut gallery = div().flex().flex_wrap().gap_2();
        for (value, label, icon) in items {
            gallery = gallery.child(
                Button::new(SharedString::from(format!("gallery-{value}")))
                    .ghost()
                    .w(px(120.))
                    .h(px(90.))
                    .border_1()
                    .border_color(rgb(if selected == value { self.palette().accent } else { self.palette().border }))
                    .child(
                        div()
                            .relative()
                            .flex()
                            .flex_col()
                            .items_center()
                            .gap_2()
                            .child(self.advanced_probe(format!("gallery:{value}"), cx))
                            .child(crate::ribbon_icons::icon(icon).size(px(38.)))
                            .child(if self.chinese() { label.to_string() } else { value.to_string() }),
                    )
                    .on_click(cx.listener(move |v, _, w, cx| v.set_dialog_value(key, json!(value), w, cx))),
            );
        }
        body = body.child(gallery);
        for field in self.command_fields.iter().filter(|f| f.key != key) {
            body = body.child(self.dialog_field(id, field, cx));
        }
        Some(body.into_any_element())
    }
}
