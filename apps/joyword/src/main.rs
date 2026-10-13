//! M0 window and physical-page projection spike, not a document editor.
//! Menus, shortcuts and on-screen buttons come from the joyword-shell command
//! registry and run through its single entry point (T106).
use gpui::{
    Action, App, Application, Bounds, Context, FocusHandle, KeyBinding, Menu, MenuItem, Window,
    WindowBounds, WindowOptions, actions, div, prelude::*, px, rgb, size,
};
use joyword_foundation::{PageSize, ScreenScale};
use joyword_shell::command::{
    Availability, CommandHost, CommandId, MenuEntry, Platform, QUIT, Registry, SWITCH_LANGUAGE,
    ZOOM_IN, ZOOM_OUT, ZOOM_RESET,
};
use joyword_shell::l10n::{Catalog, Locale};

actions!(joyword, [Quit, ZoomIn, ZoomOut, ZoomReset, SwitchLanguage]);

const ZOOM_MIN: u16 = 50;
const ZOOM_MAX: u16 = 150;
const ZOOM_DEFAULT: u16 = 80;

/// GPUI action for a registered command. Every registry entry must map;
/// `registry_commands_all_have_actions` checks it.
fn action_for(id: CommandId) -> Option<Box<dyn Action>> {
    Some(match id {
        QUIT => Box::new(Quit),
        ZOOM_IN => Box::new(ZoomIn),
        ZOOM_OUT => Box::new(ZoomOut),
        ZOOM_RESET => Box::new(ZoomReset),
        SWITCH_LANGUAGE => Box::new(SwitchLanguage),
        _ => return None,
    })
}

fn menus(registry: &Registry, catalog: &Catalog) -> Vec<Menu> {
    registry
        .menus(Platform::current())
        .into_iter()
        .map(|menu| Menu {
            name: catalog.text(menu.kind.label_key()).into(),
            items: menu
                .entries
                .into_iter()
                .filter_map(|entry| match entry {
                    MenuEntry::Separator => Some(MenuItem::separator()),
                    MenuEntry::Command(id) => {
                        let spec = registry.get(id)?;
                        Some(MenuItem::Action {
                            name: catalog.text(spec.label).into(),
                            action: action_for(id)?,
                            os_action: None,
                        })
                    }
                })
                .collect(),
        })
        .collect()
}

fn key_bindings(registry: &Registry) -> Vec<KeyBinding> {
    let platform = Platform::current();
    registry
        .commands()
        .iter()
        .filter_map(|spec| {
            let keys = spec.shortcut.for_platform(platform)?;
            Some(match spec.id {
                QUIT => KeyBinding::new(keys, Quit, None),
                ZOOM_IN => KeyBinding::new(keys, ZoomIn, None),
                ZOOM_OUT => KeyBinding::new(keys, ZoomOut, None),
                ZOOM_RESET => KeyBinding::new(keys, ZoomReset, None),
                SWITCH_LANGUAGE => KeyBinding::new(keys, SwitchLanguage, None),
                _ => return None,
            })
        })
        .collect()
}

/// Application-level commands (Quit).
struct AppHost<'a>(&'a mut App);

impl CommandHost for AppHost<'_> {
    fn availability(&self, _: CommandId) -> Availability {
        Availability::Enabled
    }

    fn execute(&mut self, id: CommandId) -> Result<(), String> {
        match id {
            QUIT => {
                self.0.quit();
                Ok(())
            }
            other => Err(format!("{other} is not an application command")),
        }
    }
}

struct JoyWord {
    zoom_percent: u16,
    locale: Locale,
    registry: Registry,
    focus_handle: FocusHandle,
}

/// View-level commands, executed with access to the view's context.
struct ViewHost<'a, 'b> {
    view: &'a mut JoyWord,
    cx: &'a mut Context<'b, JoyWord>,
}

impl CommandHost for ViewHost<'_, '_> {
    fn availability(&self, id: CommandId) -> Availability {
        match id {
            ZOOM_IN if self.view.zoom_percent >= ZOOM_MAX => Availability::Disabled("zoom.max"),
            ZOOM_OUT if self.view.zoom_percent <= ZOOM_MIN => Availability::Disabled("zoom.min"),
            _ => Availability::Enabled,
        }
    }

    fn execute(&mut self, id: CommandId) -> Result<(), String> {
        let view = &mut *self.view;
        match id {
            ZOOM_IN => view.zoom_percent = (view.zoom_percent + 10).min(ZOOM_MAX),
            ZOOM_OUT => view.zoom_percent = (view.zoom_percent - 10).max(ZOOM_MIN),
            ZOOM_RESET => view.zoom_percent = 100,
            SWITCH_LANGUAGE => {
                view.locale = match view.locale {
                    Locale::ZhCn => Locale::En,
                    Locale::En => Locale::ZhCn,
                };
                self.cx
                    .set_menus(menus(&view.registry, Catalog::shipped(view.locale)));
            }
            other => return Err(format!("{other} is not a view command")),
        }
        self.cx.notify();
        Ok(())
    }
}

impl JoyWord {
    fn label(&self, key: &str) -> String {
        Catalog::shipped(self.locale).text(key)
    }

    /// Single entry for menus, shortcuts and on-screen controls. Disabled
    /// commands (zoom limits) do nothing.
    fn run(&mut self, id: CommandId, cx: &mut Context<Self>) {
        let registry = self.registry.clone();
        let _ = registry.invoke(&mut ViewHost { view: self, cx }, id);
    }
}

impl Render for JoyWord {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let scale = ScreenScale::new(f64::from(self.zoom_percent) / 100.0)
            .expect("zoom is clamped to a valid range");
        let page = PageSize::A4;
        let width = scale.logical_pixels(page.width) as f32;
        let height = scale.logical_pixels(page.height) as f32;
        let zoom = f32::from(self.zoom_percent) / 100.0;
        let catalog = Catalog::shipped(self.locale);

        div()
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(|this, _: &ZoomIn, _, cx| this.run(ZOOM_IN, cx)))
            .on_action(cx.listener(|this, _: &ZoomOut, _, cx| this.run(ZOOM_OUT, cx)))
            .on_action(cx.listener(|this, _: &ZoomReset, _, cx| this.run(ZOOM_RESET, cx)))
            .on_action(cx.listener(|this, _: &SwitchLanguage, _, cx| this.run(SWITCH_LANGUAGE, cx)))
            .size_full()
            .flex()
            .flex_col()
            .bg(rgb(0xe9edf2))
            .text_color(rgb(0x263247))
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .px_6()
                    .py_3()
                    .bg(rgb(0x163b65))
                    .text_color(rgb(0xffffff))
                    .child("JoyWord · JoyOffice")
                    .child(
                        div()
                            .id("language")
                            .cursor_pointer()
                            .on_click(cx.listener(|this, _, _, cx| this.run(SWITCH_LANGUAGE, cx)))
                            .child(self.label("language")),
                    ),
            )
            .child(
                div()
                    .px_6()
                    .py_3()
                    .bg(rgb(0xfff8e7))
                    .text_sm()
                    .child(self.label("stage")),
            )
            .child(
                div()
                    .id("page-scroll")
                    .flex_1()
                    .min_h_0()
                    .overflow_scroll()
                    .p_8()
                    .child(
                        div()
                            .w(px(width))
                            .h(px(height))
                            .p(px(72.0 * zoom))
                            .bg(rgb(0xffffff))
                            .shadow_md()
                            .flex()
                            .flex_col()
                            .gap(px(24.0 * zoom))
                            .text_size(px(16.0 * zoom))
                            .child(
                                div()
                                    .text_size(px(32.0 * zoom))
                                    .child(self.label("welcome")),
                            )
                            .child(self.label("intro"))
                            .child("Rust + GPUI · A4 210 × 297 mm")
                            .child(self.label("projection"))
                            .child(self.label("next")),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .px_6()
                    .py_2()
                    .bg(rgb(0xffffff))
                    .text_sm()
                    .child(self.label("status"))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_4()
                            .child(
                                div()
                                    .id("zoom-out")
                                    .cursor_pointer()
                                    .px_2()
                                    .on_click(cx.listener(|this, _, _, cx| this.run(ZOOM_OUT, cx)))
                                    .child("−"),
                            )
                            .child(catalog.format(
                                "zoom.percent",
                                &[("percent", &self.zoom_percent.to_string())],
                            ))
                            .child(
                                div()
                                    .id("zoom-in")
                                    .cursor_pointer()
                                    .px_2()
                                    .on_click(cx.listener(|this, _, _, cx| this.run(ZOOM_IN, cx)))
                                    .child("+"),
                            ),
                    ),
            )
    }
}

fn main() {
    Application::new().run(|cx: &mut App| {
        let registry = Registry::builtin();
        let locale = Locale::ZhCn;
        cx.on_action({
            let registry = registry.clone();
            move |_: &Quit, cx| {
                let _ = registry.invoke(&mut AppHost(cx), QUIT);
            }
        });
        cx.bind_keys(key_bindings(&registry));
        cx.set_menus(menus(&registry, Catalog::shipped(locale)));
        let bounds = Bounds::centered(None, size(px(1100.0), px(850.0)), cx);
        let result = cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(gpui::TitlebarOptions {
                    title: Some("JoyWord · M0".into()),
                    ..Default::default()
                }),
                ..Default::default()
            },
            |_, cx| {
                cx.new(|cx| JoyWord {
                    zoom_percent: ZOOM_DEFAULT,
                    locale,
                    registry: registry.clone(),
                    focus_handle: cx.focus_handle(),
                })
            },
        );
        match result {
            Ok(window) => {
                let _ = window.update(cx, |view, window, cx| {
                    window.focus(&view.focus_handle);
                    cx.activate(true);
                });
            }
            Err(error) => {
                eprintln!("Could not open the JoyWord window: {error}");
                cx.quit();
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_commands_all_have_actions_and_bindings() {
        let registry = Registry::builtin();
        for spec in registry.commands() {
            assert!(
                action_for(spec.id).is_some(),
                "{} has no GPUI action",
                spec.id
            );
        }
        let with_keys = registry
            .commands()
            .iter()
            .filter(|spec| spec.shortcut.for_platform(Platform::current()).is_some())
            .count();
        assert_eq!(key_bindings(&registry).len(), with_keys);
    }
}
