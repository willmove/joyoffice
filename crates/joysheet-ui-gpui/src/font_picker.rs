//! Editable Ribbon font selector. Text stays in the control until confirmed.
use crate::desktop::{After, Desktop};
use gpui_kit::{
    component::{
        Disableable, Sizable,
        button::{Button, ButtonVariants},
        input::{Input, InputEvent, InputState},
        popover::Popover,
        scroll::{Scrollbar, ScrollbarMode},
    },
    prelude::*,
    *,
};
use joysheet_engine::model::Theme;
use std::collections::HashSet;

#[derive(Clone)]
pub(crate) struct FontTarget {
    pub doc: u64,
    pub sheet: usize,
    pub range: String,
}
pub(crate) struct FontPicker {
    pub input: Entity<InputState>,
    pub scroll: ScrollHandle,
    pub open: bool,
    pub original: String,
    pub filter: String,
    pub dirty: bool,
    pub navigation: Option<usize>,
    pub target: Option<FontTarget>,
}
impl FontPicker {
    pub fn new(w: &mut Window, cx: &mut App) -> Self {
        Self {
            input: cx.new(|cx| InputState::new(w, cx).default_value("Calibri")),
            scroll: ScrollHandle::new(),
            open: false,
            original: "Calibri".into(),
            filter: String::new(),
            dirty: false,
            navigation: None,
            target: None,
        }
    }
}
pub(crate) struct FontRow {
    pub name: Option<String>,
    pub label: String,
    pub group: &'static str,
}
fn font_rows(theme: &Theme, recent: &[String], installed: &[String], query: &str, zh: bool) -> Vec<FontRow> {
    let mut rows = Vec::new();
    let query = query.trim().to_lowercase();
    let mut add_group = |group, caption: &str, items: Vec<(String, String)>| {
        let items = items.into_iter().filter(|(n, _)| !n.is_empty() && n.to_lowercase().contains(&query)).collect::<Vec<_>>();
        if items.is_empty() {
            return;
        }
        rows.push(FontRow { name: None, label: caption.into(), group });
        rows.extend(items.into_iter().map(|(name, label)| FontRow { name: Some(name), label, group }));
    };
    let mut theme_names = Vec::new();
    let mut seen = HashSet::new();
    for (name, role) in [
        (&theme.major_font, if zh { "标题" } else { "Headings" }),
        (&theme.minor_font, if zh { "正文" } else { "Body" }),
        (&theme.major_font_ea, if zh { "中文标题" } else { "East Asian Headings" }),
        (&theme.minor_font_ea, if zh { "中文正文" } else { "East Asian Body" }),
        (&theme.major_font_cs, if zh { "复杂文种标题" } else { "Complex Script Headings" }),
        (&theme.minor_font_cs, if zh { "复杂文种正文" } else { "Complex Script Body" }),
    ] {
        if !name.is_empty() && seen.insert((name.to_lowercase(), role)) {
            theme_names.push((name.clone(), format!("{name}（{role}）")));
        }
    }
    add_group("theme", if zh { "主题字体" } else { "Theme Fonts" }, theme_names);
    let mut seen = HashSet::new();
    add_group(
        "recent",
        if zh { "最近使用的字体" } else { "Recently Used Fonts" },
        recent.iter().take(8).filter(|n| seen.insert(n.to_lowercase())).map(|n| (n.clone(), n.clone())).collect(),
    );
    add_group("all", if zh { "所有字体" } else { "All Fonts" }, installed.iter().map(|n| (n.clone(), n.clone())).collect());
    rows
}
fn font_name(text: &str, installed: &[String]) -> Result<String, &'static str> {
    let text = text.trim();
    if text.is_empty() || text.chars().count() > 255 || text.chars().any(char::is_control) {
        return Err("请输入有效字体名称 / Enter a valid font name");
    }
    // Excel accepts uninstalled names too; preserve them for document portability.
    Ok(installed.iter().find(|n| n.to_lowercase() == text.to_lowercase()).cloned().unwrap_or_else(|| text.into()))
}
impl Desktop {
    fn ensure_font_names(&mut self, cx: &App) {
        if self.excel_ui.fonts.is_empty() {
            self.excel_ui.fonts = cx.text_system().all_font_names();
            self.excel_ui.fonts.sort_by_key(|n| n.to_lowercase());
            self.excel_ui.fonts.dedup_by(|a, b| a.to_lowercase() == b.to_lowercase());
        }
    }
    fn font_target(&self) -> Option<FontTarget> {
        self.view().map(|v| FontTarget { doc: v.doc, sheet: v.sheet, range: self.selection().a1() })
    }
    fn current_font(&self) -> String {
        self.view()
            .and_then(|v| v.sheet().map(|s| v.workbook.styles.get(s.style_id(self.selection().active)).font.name.clone()))
            .unwrap_or("Calibri".into())
    }
    pub(crate) fn sync_font_picker(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        if !self.font_picker.open && !self.font_picker.input.read(cx).focus_handle(cx).is_focused(w) {
            let current = self.current_font();
            if self.font_picker.original != current || self.font_picker.dirty {
                self.font_picker.original = current.clone();
                self.font_picker.dirty = false;
                self.font_picker.filter.clear();
                self.font_picker.input.update(cx, |s, cx| s.set_value(current, w, cx));
            }
        }
    }
    pub(crate) fn font_input_event(&mut self, event: &InputEvent, w: &mut Window, cx: &mut Context<Self>) {
        match event {
            InputEvent::Focus => {
                self.ensure_font_names(cx);
                self.font_picker.target = self.font_target();
                self.font_picker.input.update(cx, |s, cx| s.select_all(w, cx));
            }
            InputEvent::Change => {
                let text = self.font_picker.input.read(cx).value().to_string();
                self.font_picker.dirty = text != self.font_picker.original;
                self.font_picker.filter = if self.font_picker.dirty { text } else { String::new() };
                self.font_picker.navigation = None;
                self.font_picker.scroll.set_offset(point(px(0.), px(0.)));
            }
            InputEvent::Blur if !self.font_picker.open => self.reset_font_text(w, cx),
            _ => {}
        }
        cx.notify();
    }
    fn reset_font_text(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        if self.font_picker.dirty {
            let original = self.font_picker.original.clone();
            self.font_picker.dirty = false;
            self.font_picker.input.update(cx, |s, cx| s.set_value(original, w, cx));
        }
        self.font_picker.filter.clear();
        self.font_picker.navigation = None;
    }
    fn font_rows(&self) -> Vec<FontRow> {
        let theme = self.view().map(|v| v.workbook.theme.clone()).unwrap_or_default();
        font_rows(&theme, &self.preferences.recent_fonts, &self.excel_ui.fonts, &self.font_picker.filter, self.chinese())
    }
    fn apply_ribbon_font(&mut self, name: String, return_to_grid: bool, w: &mut Window, cx: &mut Context<Self>) {
        let name = match font_name(&name, &self.excel_ui.fonts) {
            Ok(n) => n,
            Err(e) => {
                self.alert(e);
                cx.notify();
                return;
            }
        };
        let target = self.font_picker.target.clone().or_else(|| self.font_target());
        let Some(target) = target.filter(|t| self.view().is_some_and(|v| v.doc == t.doc && v.sheet == t.sheet)) else {
            self.alert("工作簿已切换，请重新选择字体 / Select a font again");
            return;
        };
        self.font_picker.open = false;
        self.excel_ui.popup_open = false;
        self.font_picker.original = name.clone();
        self.font_picker.dirty = false;
        self.font_picker.filter.clear();
        self.font_picker.navigation = None;
        self.font_picker.input.update(cx, |s, cx| s.set_value(name.clone(), w, cx));
        self.commit(After::RibbonFont(name, target), w, cx);
        if return_to_grid {
            self.focus_grid(w, cx);
        }
        cx.notify();
    }
    pub(crate) fn font_picker_key(&mut self, event: &KeyDownEvent, w: &mut Window, cx: &mut Context<Self>) {
        if self.font_picker.input.update(cx, |s, cx| s.marked_text_range(w, cx).is_some()) {
            return;
        }
        match event.keystroke.key.as_str() {
            "enter" => {
                let text = self
                    .font_picker
                    .navigation
                    .and_then(|i| self.font_rows().get(i).and_then(|r| r.name.clone()))
                    .unwrap_or_else(|| self.font_picker.input.read(cx).value().to_string());
                self.apply_ribbon_font(text, true, w, cx);
                cx.stop_propagation();
            }
            "escape" => {
                self.font_picker.open = false;
                self.excel_ui.popup_open = false;
                self.reset_font_text(w, cx);
                self.focus_grid(w, cx);
                cx.notify();
                cx.stop_propagation();
            }
            "tab" => {
                if self.font_picker.dirty {
                    let text = self.font_picker.input.read(cx).value().to_string();
                    self.apply_ribbon_font(text, false, w, cx);
                }
                self.font_picker.open = false;
                self.excel_ui.popup_open = false;
                cx.notify();
            }
            "down" | "up" => {
                self.ensure_font_names(cx);
                if self.font_picker.target.is_none() {
                    self.font_picker.target = self.font_target();
                }
                self.font_picker.open = true;
                let choices = self.font_rows().iter().enumerate().filter_map(|(i, r)| r.name.as_ref().map(|_| i)).collect::<Vec<_>>();
                let selected = self.font_picker.navigation.and_then(|i| choices.iter().position(|v| *v == i));
                let next = if event.keystroke.key == "up" {
                    selected.map(|i| i.saturating_sub(1)).unwrap_or(choices.len().saturating_sub(1))
                } else {
                    selected.map(|i| (i + 1).min(choices.len().saturating_sub(1))).unwrap_or(0)
                };
                self.font_picker.navigation = choices.get(next).copied();
                if let Some(i) = self.font_picker.navigation {
                    self.font_picker.scroll.scroll_to_item(i);
                }
                cx.notify();
                cx.stop_propagation();
            }
            _ => {}
        }
    }
    pub(crate) fn render_font_picker(&self, cx: &mut Context<Self>) -> AnyElement {
        let owner = cx.weak_entity();
        let popup_owner = owner.clone();
        let input = self.font_picker.input.clone();
        let focus = input.read(cx).focus_handle(cx);
        let enabled = self.view().is_some();
        let trigger = Button::new("font-name")
            .ghost()
            .xsmall()
            .w(px(132.))
            .h(px(24.))
            .justify_end()
            .px_1()
            .child(div().w_full().flex().justify_end().child(crate::ribbon_icons::icon(joyoffice_ui::IconName::ChevronDown).size(px(9.))))
            .accessibility_label(self.label("展开字体列表", "Open font list"))
            .disabled(!enabled)
            .tab_stop(false);
        let popup = Popover::new("ribbon-font-picker")
            .open(self.font_picker.open)
            .track_focus(&focus)
            .on_open_change(move |open, w, cx| {
                let _ = popup_owner.update(cx, |v, cx| {
                    v.font_picker.open = *open;
                    v.excel_ui.popup_open = *open;
                    if *open {
                        v.ensure_font_names(cx);
                        v.font_picker.target = v.font_target();
                        v.font_picker.scroll.set_offset(point(px(0.), px(0.)));
                    } else if !v.font_picker.input.read(cx).focus_handle(cx).is_focused(w) {
                        v.reset_font_text(w, cx);
                    }
                    cx.notify();
                });
            })
            .trigger(trigger)
            .content(move |_, _, cx| owner.update(cx, |v, cx| v.render_font_list(cx)).unwrap_or_else(|_| div().into_any_element()));
        div()
            .relative()
            .w(px(132.))
            .h(px(24.))
            .border(px(0.5))
            .border_color(rgb(self.palette().border))
            .rounded(px(3.))
            .child(popup)
            .child(div().absolute().left_0().top_0().w(px(112.)).h(px(24.)).child(self.advanced_probe("font-input".into(), cx)).child(
                Input::new(&input).xsmall().appearance(false).w_full().h_full().disabled(!enabled).aria_label(self.label("字体名称", "Font name")),
            ))
            .child(div().absolute().right_0().top_0().w(px(20.)).h_full().child(self.advanced_probe("ribbon:font-name".into(), cx)))
            .into_any_element()
    }
    fn render_font_list(&self, cx: &mut Context<Self>) -> AnyElement {
        let rows = self.font_rows();
        let height = (rows.len() as f32 * 24.).clamp(28., 360.);
        let selected = self.font_picker.navigation;
        let current = self.font_picker.input.read(cx).value();
        let p = self.palette();
        let mut list =
            div().id("font-list-scroll").h_full().w_full().overflow_y_scroll().track_scroll(&self.font_picker.scroll).pr(px(16.)).flex().flex_col();
        if rows.is_empty() {
            list = list.child(div().px_2().py_1().child(self.label("无匹配字体；按 Enter 使用输入的名称", "No match; press Enter to use this name")));
        }
        for (i, row) in rows.into_iter().enumerate() {
            if let Some(name) = row.name {
                let probe = format!("font-item:{name}");
                let group_probe = format!("font-item:{}:{name}", row.group);
                let label = row.label;
                let highlighted = selected == Some(i) || selected.is_none() && current.as_ref() == name;
                list = list.child(
                    Button::new(("font-row", i))
                        .relative()
                        .child(self.advanced_probe(probe, cx))
                        .child(self.advanced_probe(group_probe, cx))
                        .ghost()
                        .xsmall()
                        .h(px(24.))
                        .min_h(px(24.))
                        .flex_shrink_0()
                        .w_full()
                        .justify_start()
                        .bg(rgb(if highlighted { p.selected } else { p.surface }))
                        .accessibility_label(label.clone())
                        .child(div().flex_1().text_left().child(label))
                        .on_click(cx.listener(move |v, _, w, cx| v.apply_ribbon_font(name.clone(), true, w, cx))),
                );
            } else {
                list = list.child(
                    div()
                        .id(("font-heading", i))
                        .relative()
                        .h(px(24.))
                        .flex_shrink_0()
                        .px_2()
                        .text_xs()
                        .font_weight(FontWeight::SEMIBOLD)
                        .bg(rgb(p.chrome))
                        .child(self.advanced_probe(format!("font-group:{}", row.group), cx))
                        .child(row.label),
                );
            }
        }
        div()
            .id("ribbon-font-list")
            .relative()
            .w(px(300.))
            .h(px(height))
            .child(list)
            .child(
                Scrollbar::vertical(&self.font_picker.scroll)
                    .id("font-list-scrollbar")
                    .mode(ScrollbarMode::Always)
                    .styles(|s| s.track(|t| t.bg(rgb(p.chrome).into())).thumb(|t| t.bg(rgb(p.muted)))),
            )
            .child(self.advanced_probe("font-list-viewport".into(), cx))
            .child(div().absolute().right_0().top_0().w(px(16.)).h_full().child(self.advanced_probe("font-scrollbar-track".into(), cx)))
            .into_any_element()
    }
    pub(crate) fn inspect_font_picker(&self, w: &Window, cx: &App) -> serde_json::Value {
        serde_json::json!({"open":self.font_picker.open,"focused":self.font_picker.input.read(cx).focus_handle(cx).is_focused(w),"text":self.font_picker.input.read(cx).value().to_string(),"filter":self.font_picker.filter,"dirty":self.font_picker.dirty,"scrollY":f32::from(self.font_picker.scroll.offset().y),"maxScrollY":f32::from(self.font_picker.scroll.max_offset().y),"rows":self.font_rows().iter().map(|r|serde_json::json!({"name":r.name,"label":r.label,"group":r.group})).collect::<Vec<_>>()})
    }
}

#[cfg(test)]
mod tests {
    use super::{Theme, font_name, font_rows};
    #[test]
    fn theme_and_recent_fonts_precede_complete_filtered_catalog() -> Result<(), Box<dyn std::error::Error>> {
        let theme = Theme::default();
        let fonts = vec!["Arial".into(), "等线".into(), "仿宋".into()];
        let recent = vec!["仿宋".into()];
        let rows = font_rows(&theme, &recent, &fonts, "", true);
        assert_eq!(rows[0].group, "theme");
        let recent_pos = rows.iter().position(|r| r.group == "recent").ok_or("recent group")?;
        let all_pos = rows.iter().position(|r| r.group == "all").ok_or("all group")?;
        assert!(recent_pos < all_pos);
        assert_eq!(rows.iter().filter(|r| r.group == "all" && r.name.is_some()).count(), 3);
        let filtered = font_rows(&theme, &recent, &fonts, "仿", true);
        assert!(filtered.iter().filter_map(|r| r.name.as_ref()).all(|n| n == "仿宋"));
        assert_eq!(font_name("  arial ", &fonts)?, "Arial");
        assert_eq!(font_name("Portable Font", &fonts)?, "Portable Font");
        assert!(font_name(" ", &fonts).is_err());
        Ok(())
    }
}
