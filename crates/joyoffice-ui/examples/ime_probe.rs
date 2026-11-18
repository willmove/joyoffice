//! A single stock GPUI input for comparing the system IME with the product editors.
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::{prelude::*, *};

struct Probe {
    input: Entity<InputState>,
}

impl Render for Probe {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().p_4().child(Input::new(&self.input))
    }
}

fn main() {
    gpui_kit::application()
        .with_assets(gpui_kit::assets::AllAssets)
        .run(|cx| {
            gpui_kit::init(cx);
            cx.on_window_closed(|cx, _| {
                if cx.windows().is_empty() {
                    cx.quit();
                }
            })
            .detach();
            let opened = gpui_kit::open_window(WindowOptions::default(), cx, |window, cx| {
                window.set_window_title("JoyOffice IME probe");
                let input = cx.new(|cx| InputState::new(window, cx).placeholder("请输入中文"));
                window.focus(&input.focus_handle(cx), cx);
                cx.new(|cx: &mut Context<Probe>| {
                    cx.observe_in(&input, window, |_, input, window, cx| {
                        let marked =
                            input.update(cx, |state, cx| state.marked_text_range(window, cx));
                        let text = input.read(cx).value();
                        eprintln!(
                            "ime-probe focused={} marked={} utf16={} chinese={}",
                            input.focus_handle(cx).is_focused(window),
                            marked.is_some(),
                            text.encode_utf16().count(),
                            text.as_ref() == "中文"
                        );
                    })
                    .detach();
                    Probe { input }
                })
            });
            if let Err(error) = opened {
                eprintln!("IME probe window: {error}");
                std::process::exit(1);
            }
            cx.activate(true);
        });
}
