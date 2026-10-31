use crate::desktop::{After, Desktop};
use gpui_kit::{component::button::Button, prelude::*, *};
use serde_json::{Value, json};

impl Desktop {
    pub fn set_options(&mut self, params: &Value, cx: &mut Context<Self>) -> Result<Value, String> {
        let mut prefs = self.preferences.clone();
        for key in ["dark", "autosave", "formulaBar", "autocomplete", "recovery", "animations"] {
            if let Some(v) = params.get(key) {
                let on = v.as_bool().ok_or_else(|| format!("{key} must be true or false"))?;
                match key {
                    "dark" => prefs.dark = on,
                    "autosave" => prefs.autosave = on,
                    "formulaBar" => prefs.formula_bar = on,
                    "autocomplete" => prefs.autocomplete = on,
                    "recovery" => prefs.recovery = on,
                    _ => {
                        if on {
                            return Err("Animated transitions are not available in this build".into());
                        }
                        prefs.animations = false;
                    }
                }
            }
        }
        if let Some(language) = params.get("language") {
            let language = language.as_str().filter(|v| ["zh", "en"].contains(v)).ok_or("language must be zh or en")?;
            prefs.language = language.into();
        }
        if params.get("ribbonCollapsed").is_some() {
            return Err("The GPUI command strip is not collapsible".into());
        }
        self.preferences = prefs;
        self.autosave_revision = None;
        gpui_kit::component::Theme::change(
            if self.preferences.dark { gpui_kit::component::ThemeMode::Dark } else { gpui_kit::component::ThemeMode::Light },
            None,
            cx,
        );
        if params.get("persist").and_then(Value::as_bool) != Some(false) {
            self.persist_preferences(cx);
        } else {
            cx.set_global(crate::SharedPreferences(self.preferences.clone()));
        }
        cx.notify();
        Ok(
            json!({"dark":self.preferences.dark,"autosave":self.preferences.autosave,"formulaBar":self.preferences.formula_bar,"autocomplete":self.preferences.autocomplete,"recovery":self.preferences.recovery,"animations":self.preferences.animations,"language":self.preferences.language}),
        )
    }
    pub fn settings_panel(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let mut panel = div()
            .id("settings-panel")
            .role(Role::Dialog)
            .aria_label("Joysheet settings")
            .absolute()
            .right_0()
            .top(px(54.))
            .bottom_0()
            .w(px(410.))
            .p_4()
            .bg(rgb(if self.preferences.dark { 0x20332a } else { 0xffffff }))
            .occlude()
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .gap_3()
            .child(Button::new("close-settings").label(self.label("设置 · 关闭 ×", "Settings · Close ×")).on_click(cx.listener(|this, _, w, cx| {
                this.daily_group = 0;
                this.focus_grid(w, cx);
                cx.notify();
            })));
        for (key, on, zh, en) in [
            ("dark", self.preferences.dark, "深色主题", "Dark theme"),
            ("autosave", self.preferences.autosave, "自动保存已有文件", "Save existing files automatically"),
            ("formulaBar", self.preferences.formula_bar, "显示公式栏", "Show formula bar"),
            ("autocomplete", self.preferences.autocomplete, "公式自动补全", "Formula completion"),
            ("recovery", self.preferences.recovery, "保留崩溃恢复副本", "Keep recovery copies"),
        ] {
            let label = format!("{} {}", if on { "✓" } else { "○" }, self.label(zh, en));
            panel =
                panel.child(Button::new(SharedString::from(format!("setting-{key}"))).label(label).on_click(cx.listener(move |this, _, _, cx| {
                    if let Err(e) = this.set_options(&json!({key:!on}), cx) {
                        this.alert(e);
                    }
                })));
        }
        panel = panel.child(Button::new("settings-language").label(self.label("界面语言：中文 → English", "Language: English → 中文")).on_click(
            cx.listener(|this, _, _, cx| {
                let next = if this.chinese() { "en" } else { "zh" };
                let _ = this.set_options(&json!({"language":next}), cx);
            }),
        ));
        panel = panel.child(Button::new("open-recovery").label("恢复其他会话的副本… / Recovery copies…").on_click(cx.listener(|this, _, _, cx| {
            this.daily_group = 21;
            cx.notify();
        })));
        panel = panel.child(Button::new("open-with-password").label(self.label("打开含密码的工作簿…", "Open workbook with password…")).on_click(
            cx.listener(|this, _, w, cx| {
                this.open_command("file.open", w, cx);
            }),
        ));
        panel=panel.child(Button::new("calculation-settings").label(self.label("计算模式与迭代参数…","Calculation and iteration…")).on_click(cx.listener(|this,_,w,cx|{
            let p=this.view().map(|v|json!({"mode":match v.workbook.calc.mode{joysheet_engine::model::CalcMode::Manual=>"manual",joysheet_engine::model::CalcMode::AutomaticExceptTables=>"automaticExceptTables",_=>"automatic"},"iterative":v.workbook.calc.iterative,"maxIterations":v.workbook.calc.max_iterations,"maxChange":v.workbook.calc.max_change})).unwrap_or_else(||json!({}));
            this.open_prefilled("formulas.calculationOptions",p,w,cx);
        })));
        panel
            .child(Button::new("clear-recent").label(self.label("清空最近文件列表", "Clear recent files")).on_click(cx.listener(|this, _, _, cx| {
                this.preferences.recent.clear();
                this.persist_preferences(cx);
                cx.notify();
            })))
            .child(
                Button::new("recalculate")
                    .label(self.label("重新计算工作簿", "Recalculate workbook"))
                    .on_click(cx.listener(|this, _, w, cx| this.commit(After::Command("formulas.calculateNow".into(), json!({})), w, cx))),
            )
    }
}
