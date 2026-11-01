//! Familiar native ribbon pickers backed by the existing document commands.
use super::*;
use gpui_kit::{
    assets::IconName as I,
    component::{
        Sizable,
        button::ButtonVariants,
        menu::{DropdownMenu, PopupMenuItem},
    },
};
impl Editor {
    pub(super) fn font_menu(&self, cx: &mut Context<Self>) -> AnyElement {
        let weak = cx.entity().downgrade();
        let family = self.fields[12].read(cx).value();
        let family = if family == "sans-serif" {
            "默认字体".to_owned()
        } else {
            family.to_string()
        };
        Button::new("font-picker")
            .ghost()
            .small()
            .h(px(joyoffice_ui::metrics::CONTROL))
            .w(px(144.))
            .label(family)
            .icon(I::ChevronDown)
            .accessibility_id("font-picker")
            .accessibility_label("字体列表")
            .tooltip("字体列表")
            .dropdown_menu(move |mut menu, w, _| {
                menu = menu.scrollable(true);
                for name in w.text_system().all_font_names().into_iter().take(512) {
                    let weak = weak.clone();
                    menu = menu.item(PopupMenuItem::new(name.clone()).on_click(move |_, w, cx| {
                        let _ = weak.update(cx, |s, cx| {
                            s.fields[12].update(cx, |f, cx| f.set_value(name.clone(), w, cx));
                            s.home_text("font", w, cx);
                        });
                    }));
                }
                menu
            })
            .into_any_element()
    }
    pub(super) fn shape_menu(&self, cx: &mut Context<Self>) -> AnyElement {
        let weak = cx.entity().downgrade();
        ui::large_tool("shape-gallery", "形状", I::Shapes)
            .dropdown_menu(move |mut menu, _, _| {
                for (kind, name) in [
                    ("rect", "矩形"),
                    ("roundRect", "圆角矩形"),
                    ("ellipse", "椭圆"),
                    ("line", "直线"),
                    ("arrow", "箭头"),
                    ("text", "文本框"),
                ] {
                    let weak = weak.clone();
                    menu = menu.item(PopupMenuItem::new(name).on_click(move |_, _, cx| {
                        let _ = weak.update(cx, |s, cx| {
                            s.command(Command::AddObject(s.page, edit::object(kind)), cx)
                        });
                    }));
                }
                menu
            })
            .into_any_element()
    }
    pub(super) fn table_dialog(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        self.finish_text(cx);
        let weak = cx.entity().downgrade();
        let rows = self.table_fields[0].clone();
        let cols = self.table_fields[1].clone();
        w.open_dialog(cx, move |d, _, _| {
            let weak = weak.clone();
            d.title("插入表格")
                .child(ui::label("设置表格的行数和列数"))
                .child(
                    div()
                        .flex()
                        .gap_3()
                        .child(ui::field("列数", &cols).flex_1())
                        .child(ui::field("行数", &rows).flex_1()),
                )
                .child(
                    div()
                        .flex()
                        .justify_end()
                        .gap_2()
                        .pt_4()
                        .border_t_1()
                        .border_color(rgb(ui::PALETTE.border))
                        .child(
                            Button::new("insert-table-cancel")
                                .accessibility_id("insert-table-cancel")
                                .label("取消")
                                .on_click(|_, w, cx| w.close_dialog(cx)),
                        )
                        .child(
                            Button::new("insert-table-confirm")
                                .accessibility_id("insert-table-confirm")
                                .primary()
                                .label("插入")
                                .on_click(move |_, w, cx| {
                                    w.close_dialog(cx);
                                    let _ = weak.update(cx, |s, cx| s.insert_table(w, cx));
                                }),
                        ),
                )
        });
    }
}

#[derive(Clone, Copy)]
pub(super) enum ColorTarget {
    Text,
    Fill,
    Stroke,
    Accent,
    Background,
}
impl Editor {
    pub(super) fn color_input(&self, target: ColorTarget) -> &Entity<InputState> {
        match target {
            ColorTarget::Text => &self.fields[13],
            ColorTarget::Fill => &self.fields[4],
            ColorTarget::Stroke => &self.fields[8],
            ColorTarget::Accent => &self.doc_fields[7],
            ColorTarget::Background => &self.doc_fields[11],
        }
    }
    pub(super) fn color_field(
        &self,
        id: &'static str,
        name: &'static str,
        target: ColorTarget,
        cx: &mut Context<Self>,
    ) -> Div {
        joyoffice_ui::field(ui::PALETTE, name).child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .child(self.color_menu(id, "", target, cx))
                .child(
                    Input::new(self.color_input(target))
                        .aria_label(format!("{name} · 精确值"))
                        .h(px(30.))
                        .w(px(170.)),
                ),
        )
    }
    pub(super) fn color_menu(
        &self,
        id: &'static str,
        name: &'static str,
        target: ColorTarget,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let weak = cx.entity().downgrade();
        let value = self.color_input(target).read(cx).value();
        let mut colors: Vec<(String, String)> = vec![
            ("墨黑".into(), "#26352d".into()),
            ("白色".into(), "#ffffff".into()),
            ("珊瑚".into(), "#a63d24".into()),
            ("海蓝".into(), "#315fa8".into()),
            ("青绿".into(), "#087f8c".into()),
            ("森林".into(), "#146c43".into()),
            ("紫罗兰".into(), "#7953a8".into()),
            ("金色".into(), "#ad7900".into()),
        ];
        if let Some(theme) = self.session.deck.slides[self.page]
            .layout
            .as_ref()
            .and_then(|id| self.session.deck.design.layouts.get(id))
            .and_then(|l| self.session.deck.design.masters.get(&l.master))
            .and_then(|m| self.session.deck.design.themes.get(&m.theme))
        {
            for (key, value) in &theme.colors {
                colors.push((format!("主题 · {key}"), value.clone()));
            }
        }
        ui::tool(id, name, I::ChevronDown)
            .accessibility_label(match target {
                ColorTarget::Text => "文字颜色",
                ColorTarget::Fill => "填充颜色",
                ColorTarget::Stroke => "边框颜色",
                ColorTarget::Accent => "主题强调色",
                ColorTarget::Background => "主题背景色",
            })
            .child(ui::swatch(&value))
            .dropdown_menu(move |mut menu, _, _| {
                for (name, value) in &colors {
                    let weak = weak.clone();
                    let value = value.clone();
                    menu = menu.item(PopupMenuItem::new(format!("{name}  {value}")).on_click(
                        move |_, w, cx| {
                            let _ = weak.update(cx, |s, cx| {
                                s.color_input(target)
                                    .clone()
                                    .update(cx, |f, cx| f.set_value(value.clone(), w, cx));
                                if matches!(target, ColorTarget::Text) {
                                    s.home_text("style", w, cx);
                                }
                                cx.notify();
                            });
                        },
                    ));
                }
                menu
            })
            .into_any_element()
    }
}
