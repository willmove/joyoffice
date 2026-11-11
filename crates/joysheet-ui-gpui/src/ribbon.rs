use crate::desktop::{After, Desktop};
use crate::ribbon_catalog::TABS;
use gpui_kit::{prelude::*, *};
use serde_json::json;

impl Desktop {
    pub fn render_document_header(&self, cx: &mut Context<Self>) -> AnyElement {
        let title = self.view().map(|v| v.title.clone()).unwrap_or_else(|| self.label("工作簿", "Workbook"));
        div()
            .h(px(30.))
            .flex_shrink_0()
            .px_4()
            .flex()
            .items_center()
            .gap_3()
            .bg(rgb(if self.preferences.dark { self.palette().header } else { 0xebedef }))
            .child(div().font_weight(FontWeight::BOLD).text_color(rgb(self.palette().accent)).child("Joysheet"))
            .child(self.toolbar_button("new", self.label("新建", "New"), "file.new", json!({}), cx))
            .child(div().id("open").cursor_pointer().child(self.label("打开", "Open")).on_click(cx.listener(|this, _, w, cx| this.pick_open(w, cx))))
            .child(
                div()
                    .id("save")
                    .role(Role::Button)
                    .aria_label(self.label("保存", "Save"))
                    .cursor_pointer()
                    .px_2()
                    .child(self.label("保存", "Save"))
                    .on_click(cx.listener(|this, _, w, cx| this.commit(After::Save(false), w, cx))),
            )
            .child(self.toolbar_button("undo", "↶", "edit.undo", json!({}), cx))
            .child(self.toolbar_button("redo", "↷", "edit.redo", json!({}), cx))
            .child(div().w(px(0.5)).h(px(18.)).bg(rgb(self.palette().border)))
            .child(div().max_w(px(280.)).overflow_hidden().font_weight(FontWeight::SEMIBOLD).child(title))
            .when(self.view().is_some_and(|v| v.dirty), |el| el.child(div().text_color(rgb(self.palette().accent)).child("●")))
            .child(div().flex_1())
            .child(
                div()
                    .id("command-palette")
                    .role(Role::Button)
                    .aria_label(self.label("搜索命令", "Search commands"))
                    .w(px(236.))
                    .px_3()
                    .py_1()
                    .rounded_md()
                    .bg(rgb(self.palette().chrome))
                    .cursor_pointer()
                    .text_color(rgb(self.palette().muted))
                    .child(self.label("搜索命令   Ctrl+K", "Search commands   Ctrl+K"))
                    .on_click(cx.listener(|this, _, w, cx| {
                        this.palette_open = true;
                        this.search.update(cx, |s, cx| s.focus(w, cx));
                        cx.notify();
                    })),
            )
            .child(
                div()
                    .id("autosave")
                    .cursor_pointer()
                    .text_xs()
                    .child(if self.preferences.autosave {
                        self.label("自动保存：开", "AutoSave: on")
                    } else {
                        self.label("自动保存：关", "AutoSave: off")
                    })
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.preferences.autosave = !this.preferences.autosave;
                        this.autosave_revision = None;
                        this.persist_preferences(cx);
                        cx.notify();
                    })),
            )
            .child(
                joyoffice_ui::control(self.palette(), true, self.daily_group == 20)
                    .id("open-settings")
                    .role(Role::Button)
                    .aria_label(self.label("设置", "Settings"))
                    .tab_stop(true)
                    .cursor_pointer()
                    .child(self.label("设置", "Settings"))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.daily_group = 20;
                        cx.notify();
                    }))
                    .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                        if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                            this.daily_group = 20;
                            cx.notify();
                            cx.stop_propagation();
                        }
                    })),
            )
            .child(div().id("language").cursor_pointer().child(if self.chinese() { "EN" } else { "中文" }).on_click(cx.listener(|this, _, _, cx| {
                this.preferences.language = if this.chinese() { "en".into() } else { "zh".into() };
                this.persist_preferences(cx);
                cx.notify();
            })))
            .child(div().id("theme").cursor_pointer().child(if self.preferences.dark { "☀" } else { "◐" }).on_click(cx.listener(|this, _, _, cx| {
                if let Err(error) = this.set_options(&json!({"dark":!this.preferences.dark}), cx) {
                    this.alert(error);
                }
                cx.notify();
            })))
            .into_any_element()
    }
    pub fn render_ribbon_tabs(&self, cx: &mut Context<Self>) -> AnyElement {
        div()
            .id("ribbon-tabs")
            .h(px(30.))
            .flex_shrink_0()
            .flex()
            .items_center()
            .px_2()
            .gap_1()
            .role(Role::TabList)
            .aria_label(self.label("功能区", "Ribbon"))
            .bg(rgb(self.palette().chrome))
            .children(TABS.iter().enumerate().map(|(tab, &(id, zh, en))| {
                let active = self.ribbon_tab == tab;
                div()
                    .id(id)
                    .role(Role::Tab)
                    .aria_label(self.label(zh, en))
                    .aria_selected(active)
                    .tab_stop(true)
                    .h_full()
                    .px(px(11.))
                    .flex()
                    .items_center()
                    .cursor_pointer()
                    .border_b(px(1.))
                    .border_color(rgb(if active { self.palette().accent } else { self.palette().chrome }))
                    .when(active, |el| el.font_weight(FontWeight::SEMIBOLD).text_color(rgb(self.palette().accent)))
                    .relative()
                    .child(self.advanced_probe(format!("ribbon-tab:{en}"), cx))
                    .child(self.label(zh, en))
                    .on_key_down(cx.listener(move |v, e: &KeyDownEvent, _, cx| {
                        match e.keystroke.key.as_str() {
                            "enter" | "space" => v.ribbon_tab = tab,
                            "left" => v.ribbon_tab = (v.ribbon_tab + TABS.len() - 1) % TABS.len(),
                            "right" => v.ribbon_tab = (v.ribbon_tab + 1) % TABS.len(),
                            "home" => v.ribbon_tab = 1,
                            "end" => v.ribbon_tab = TABS.len() - 1,
                            _ => return,
                        }
                        v.daily_group = 0;
                        cx.stop_propagation();
                        cx.notify();
                    }))
                    .on_click(cx.listener(move |v, e: &ClickEvent, w, cx| {
                        v.ribbon_tab = tab;
                        v.daily_group = 0;
                        v.help_open = false;
                        v.function_category = None;
                        if e.click_count() > 1 && tab != 0 {
                            v.ribbon_collapsed = !v.ribbon_collapsed;
                        }
                        v.focus_grid(w, cx);
                        cx.notify();
                    }))
            }))
            .child(div().flex_1())
            .child(
                div()
                    .id("more")
                    .px_2()
                    .cursor_pointer()
                    .aria_label(self.label("搜索命令", "Search Commands"))
                    .child(crate::ribbon_icons::icon(joyoffice_ui::IconName::Search).size(px(16.)))
                    .on_click(cx.listener(|v, _, w, cx| {
                        v.palette_open = true;
                        v.search.update(cx, |s, cx| s.focus(w, cx));
                        cx.notify();
                    })),
            )
            .child(
                div()
                    .id("ribbon-collapse")
                    .px_2()
                    .cursor_pointer()
                    .aria_label(self.label("折叠功能区", "Collapse Ribbon"))
                    .child(
                        crate::ribbon_icons::icon(if self.ribbon_collapsed {
                            joyoffice_ui::IconName::ChevronDown
                        } else {
                            joyoffice_ui::IconName::ChevronUp
                        })
                        .size(px(14.)),
                    )
                    .on_click(cx.listener(|v, _, _, cx| {
                        v.ribbon_collapsed = !v.ribbon_collapsed;
                        cx.notify();
                    })),
            )
            .into_any_element()
    }
    pub fn render_ribbon_tools(&self, cx: &mut Context<Self>) -> AnyElement {
        self.excel_ribbon_tools(cx)
    }
}
