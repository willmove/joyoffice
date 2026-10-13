//! M0 window and physical-page projection spike, not a document editor.
use gpui::{
    App, Application, Bounds, Context, Window, WindowBounds, WindowOptions, actions, div,
    prelude::*, px, rgb, size,
};
use joyword_foundation::{PageSize, ScreenScale};

actions!(joyword, [Quit]);

struct JoyWord {
    zoom_percent: u16,
    english: bool,
}

impl JoyWord {
    fn label(&self, key: &'static str) -> &'static str {
        // M0 has no interpolation or plural rules. Resource parity is checked in CI.
        let resource = if self.english {
            include_str!("../locales/en.lang")
        } else {
            include_str!("../locales/zh-CN.lang")
        };
        resource
            .lines()
            .find_map(|line| {
                let (name, value) = line.split_once('=')?;
                (name == key).then_some(value)
            })
            .unwrap_or(key)
    }

    fn change_zoom(&mut self, delta: i32, cx: &mut Context<Self>) {
        self.zoom_percent = (i32::from(self.zoom_percent) + delta).clamp(50, 150) as u16;
        cx.notify();
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

        div()
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
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.english = !this.english;
                                cx.notify();
                            }))
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
                                    .on_click(
                                        cx.listener(|this, _, _, cx| this.change_zoom(-10, cx)),
                                    )
                                    .child("−"),
                            )
                            .child(format!("{}%", self.zoom_percent))
                            .child(
                                div()
                                    .id("zoom-in")
                                    .cursor_pointer()
                                    .px_2()
                                    .on_click(
                                        cx.listener(|this, _, _, cx| this.change_zoom(10, cx)),
                                    )
                                    .child("+"),
                            ),
                    ),
            )
    }
}

fn main() {
    Application::new().run(|cx: &mut App| {
        cx.on_action(|_: &Quit, cx| cx.quit());
        cx.bind_keys([gpui::KeyBinding::new(
            if cfg!(target_os = "macos") {
                "cmd-q"
            } else {
                "ctrl-q"
            },
            Quit,
            None,
        )]);
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
                cx.new(|_| JoyWord {
                    zoom_percent: 80,
                    english: false,
                })
            },
        );
        if let Err(error) = result {
            eprintln!("Could not open the JoyWord window: {error}");
            cx.quit();
        } else {
            cx.activate(true);
        }
    });
}
