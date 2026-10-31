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
        Button::new("font-picker")
            .ghost()
            .small()
            .icon(I::ChevronDown)
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
        chrome::large_tool("shape-gallery", "形状", I::Shapes)
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
                .child("列数")
                .child(Input::new(&cols))
                .child("行数")
                .child(Input::new(&rows))
                .child(
                    Button::new("insert-table-confirm")
                        .primary()
                        .label("插入")
                        .on_click(move |_, w, cx| {
                            w.close_dialog(cx);
                            let _ = weak.update(cx, |s, cx| s.insert_table(w, cx));
                        }),
                )
                .child(
                    Button::new("insert-table-cancel")
                        .label("取消")
                        .on_click(|_, w, cx| w.close_dialog(cx)),
                )
        });
    }
}
