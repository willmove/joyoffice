#![forbid(unsafe_code)]
use gpui_kit::component::{
    Theme, ThemeMode, WindowExt,
    button::Button,
    input::{Input, InputState},
    menu::DropdownMenu,
};
use gpui_kit::*;
mod editor;
mod jobs;
mod slideshow;
mod viewer;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

actions!(
    joydeck,
    [
        Quit,
        ToggleTheme,
        ToggleLanguage,
        SecondWindow,
        About,
        NewWindow,
        OpenDocument,
        UndoFocused,
        RedoFocused
    ]
);

struct Diagnostics {
    input: Entity<InputState>,
    focus: FocusHandle,
    dark: bool,
    chinese: bool,
    scale: f32,
    progress: u32,
    running: bool,
    cancel: Arc<AtomicBool>,
    task: Option<Task<()>>,
    image: Arc<RenderImage>,
}

impl Drop for Diagnostics {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
}

impl Diagnostics {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let input = cx.new(|cx| {
            InputState::new(window, cx).placeholder("输入中文 / Type here · IME, emoji, Ctrl+Z")
        });
        // GPUI RenderImage uses BGRA bytes; image::Frame is only the storage carrier.
        let pixels = image::RgbaImage::from_fn(160, 100, |x, y| {
            image::Rgba([200, (x * 255 / 160) as u8, 40, (y * 255 / 100) as u8])
        });
        let focus = cx.focus_handle();
        focus.focus(window, cx);
        Self {
            input,
            focus,
            dark: true,
            chinese: true,
            scale: 1.,
            progress: 0,
            running: false,
            cancel: Arc::new(AtomicBool::new(false)),
            task: None,
            image: Arc::new(RenderImage::new(vec![image::Frame::new(pixels)])),
        }
    }
    fn word(&self, zh: &'static str, en: &'static str) -> &'static str {
        if self.chinese { zh } else { en }
    }
    fn toggle_theme(&mut self, _: &ToggleTheme, window: &mut Window, cx: &mut Context<Self>) {
        self.dark = !self.dark;
        Theme::change(
            if self.dark {
                ThemeMode::Dark
            } else {
                ThemeMode::Light
            },
            Some(window),
            cx,
        );
        cx.notify();
    }
    fn toggle_language(&mut self, _: &ToggleLanguage, _: &mut Window, cx: &mut Context<Self>) {
        self.chinese = !self.chinese;
        cx.notify();
    }
    fn second(&mut self, _: &SecondWindow, _: &mut Window, cx: &mut Context<Self>) {
        if let Err(e) = open_diagnostics(cx) {
            eprintln!("Second window failed: {e}");
        }
    }
    fn about(&mut self, _: &About, window: &mut Window, cx: &mut Context<Self>) {
        window.open_dialog(cx, |dialog, _, _| dialog.title("JoyDeck · M0").child("Rust 1.98.1 · GPUI Kit 0.8.1 / gpui-pre 0.3.10. Native diagnostics; M1 provides read-only document browsing."));
    }
    fn start(&mut self, cx: &mut Context<Self>) {
        self.cancel.store(true, Ordering::Relaxed);
        let cancel = Arc::new(AtomicBool::new(false));
        self.cancel = cancel.clone();
        self.running = true;
        self.progress = 0;
        // Each CPU chunk is owned background work. A weak entity prevents late delivery to a closed window.
        self.task = Some(cx.spawn(async move |view, cx| {
            for progress in 1..=100 {
                let stop = cancel.clone();
                let work = cx
                    .background_executor()
                    .spawn(async move { jobs::chunk(&stop) })
                    .await;
                if !work {
                    break;
                }
                if view
                    .update(cx, |this, cx| {
                        this.progress = progress;
                        cx.notify();
                    })
                    .is_err()
                {
                    break;
                }
            }
            let _ = view.update(cx, |this, cx| {
                this.running = false;
                eprintln!("Diagnostic worker settled at {}%", this.progress);
                cx.notify();
            });
        }));
        cx.notify();
    }
}

impl Render for Diagnostics {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let bg = if self.dark { 0x101820 } else { 0xf3f5f7 };
        let panel = if self.dark { 0x1a2633 } else { 0xffffff };
        let fg = if self.dark { 0xe5edf5 } else { 0x192735 };
        let scale = self.scale;
        let preview = canvas(
            |bounds, _, _| bounds,
            move |bounds, _, window, _| {
                let mut path = PathBuilder::fill();
                let p = |x, y| {
                    point(
                        bounds.origin.x + px(x * scale),
                        bounds.origin.y + px(y * scale),
                    )
                };
                path.move_to(p(20., 110.));
                path.line_to(p(90., 20.));
                path.line_to(p(160., 110.));
                path.close();
                if let Ok(path) = path.build() {
                    window.paint_path(path, rgb(0x38bdb2));
                }
            },
        )
        .w(px(200.))
        .h(px(160.));
        div().id("diagnostics").track_focus(&self.focus).overflow_y_scroll().key_context("Diagnostics").size_full().flex().flex_col().gap_3().p_6()
            .bg(rgb(bg)).text_color(rgb(fg)).font_family(if cfg!(target_os = "windows") { "Microsoft YaHei" } else if cfg!(target_os = "macos") { "Helvetica" } else { "DejaVu Sans" })
            .on_action(cx.listener(Self::toggle_theme)).on_action(cx.listener(Self::toggle_language))
            .on_action(cx.listener(Self::second)).on_action(cx.listener(Self::about))
            .child(div().flex().justify_between().items_center()
                .child(div().flex().flex_col().gap_2().child(div().text_3xl().child("JoyDeck"))
                    .child(self.word("M0 · 原生桌面技术预览", "M0 · Native desktop diagnostics")))
                .child(div().flex().gap_2()
                    .child(Button::new("menu").label(self.word("菜单", "Menu")).dropdown_menu(|menu, _, _| menu.menu("Theme / 明暗", Box::new(ToggleTheme)).menu("Language / 语言", Box::new(ToggleLanguage)).menu("New Window / 新窗口", Box::new(SecondWindow)).menu("About / 关于", Box::new(About)).menu("Quit / 退出", Box::new(Quit))))
                    .child(Button::new("theme").label(self.word("切换明暗", "Theme")).on_click(cx.listener(|s, _, w, cx| s.toggle_theme(&ToggleTheme, w, cx))))
                    .child(Button::new("language").label("中 / EN").on_click(cx.listener(|s, _, w, cx| s.toggle_language(&ToggleLanguage, w, cx))))
                    .child(Button::new("second").label(self.word("第二窗口", "New window")).on_click(cx.listener(|s, _, w, cx| s.second(&SecondWindow, w, cx))))
                    .child(Button::new("about").label(self.word("关于", "About")).on_click(cx.listener(|s, _, w, cx| s.about(&About, w, cx))))))
            .child(div().rounded_lg().p_4().bg(rgb(panel)).flex().flex_col().gap_3()
                .child(self.word("01  输入与焦点", "01  Input & focus"))
                .child(Input::new(&self.input))
                .child(self.word("测试预编辑、提交、取消、选择与撤销。Tab 切换焦点，Ctrl+Z 撤销。", "Test composition, commit, cancel, selection and undo. Tab changes focus; Ctrl+Z undoes.")))
            .child(div().rounded_lg().p_4().bg(rgb(panel)).flex().flex_col().gap_3()
                .child(self.word("02  GPUI 路径与 CPU 图像桥", "02  GPUI paths & CPU image bridge"))
                .child(div().flex().items_center().gap_6().overflow_hidden().h(px(165.)).child(preview)
                    .child(div().bg(rgb(0xdde3eb)).child(img(self.image.clone()).w(px(160. * scale)).h(px(100. * scale)))))
                .child(div().flex().gap_3().items_center()
                    .child(Button::new("minus").label("−").on_click(cx.listener(|s, _, _, cx| { s.scale = (s.scale - 0.25).max(0.5); cx.notify(); })))
                    .child(format!("{:.0}% · clipped viewport · BGRA / alpha", self.scale * 100.))
                    .child(Button::new("plus").label("+").on_click(cx.listener(|s, _, _, cx| { s.scale = (s.scale + 0.25).min(1.5); cx.notify(); })))))
            .child(div().rounded_lg().p_4().bg(rgb(panel)).flex().flex_col().gap_3()
                .child(self.word("03  后台任务生命周期", "03  Background task lifetime"))
                .child(div().flex().items_center().gap_3()
                    .child(Button::new("start").label(self.word("启动 / 重启", "Start / Restart")).on_click(cx.listener(|s, _, _, cx| s.start(cx))))
                    .child(Button::new("cancel").label(self.word("取消任务", "Cancel")).on_click(cx.listener(|s, _, _, cx| { s.cancel.store(true, Ordering::Relaxed); s.running = false; s.task = None; cx.notify(); })))
                    .child(format!("{}% · {}", self.progress, if self.running { self.word("运行中", "Running") } else { self.word("空闲", "Idle") }))))
            .child(div().text_sm().child(format!("v{} · {} / {} · DPI scale {:.2} · GPUI Kit 0.8.1 / pre 0.3.10", env!("CARGO_PKG_VERSION"), std::env::consts::OS, std::env::consts::ARCH, window.scale_factor())))
            .child(div().text_sm().child(self.word("Ctrl+Shift+T 明暗  ·  Ctrl+Shift+L 语言  ·  Ctrl+Shift+N 新窗口  ·  Ctrl+Q 退出", "Ctrl+Shift+T theme · Ctrl+Shift+L language · Ctrl+Shift+N new window · Ctrl+Q quit")))
    }
}

fn open_diagnostics(cx: &mut App) -> Result<(AnyWindowHandle, Entity<Diagnostics>)> {
    gpui_kit::open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                None,
                size(px(1000.), px(720.)),
                cx,
            ))),
            ..Default::default()
        },
        cx,
        |window, cx| {
            window.set_window_title("JoyDeck — M0 Diagnostics");
            cx.new(|cx| Diagnostics::new(window, cx))
        },
    )
}
/// Exercise the real production entities on a native window. This validates
/// the adapter API, not the physical OS input method or mouse event injection.
fn smoke(window: &mut Window, view: &Entity<Diagnostics>, cx: &mut App) {
    view.update(cx, |this, cx| {
        this.toggle_theme(&ToggleTheme, window, cx);
        this.toggle_language(&ToggleLanguage, window, cx);
        this.input.update(cx, |input, cx| {
            input.set_value("", window, cx);
            input.replace_and_mark_text_in_range(None, "输入", Some(0..2), window, cx);
            input.replace_text_in_range(None, "中文😀", window, cx);
            input.replace_text_in_range(Some(2..4), "✅", window, cx);
            let committed = input.value();
            eprintln!(
                "SMOKE utf16_commit={} value={committed}",
                committed.as_ref() == "中文✅"
            );
            input.set_value("", window, cx);
            input.replace_and_mark_text_in_range(None, "临时", None, window, cx);
            input.replace_text_in_range(None, "", window, cx);
            eprintln!("SMOKE composition_cancel={}", input.value().is_empty());
            input.set_value("JoyDeck 中文输入 😀", window, cx);
        });
        this.toggle_language(&ToggleLanguage, window, cx);
        this.toggle_theme(&ToggleTheme, window, cx);
        this.about(&About, window, cx);
        eprintln!("SMOKE modal_open={}", window.has_active_dialog(cx));
        window.close_dialog(cx);
        this.start(cx);
        cx.notify();
    });
    if let Ok((second, second_view)) = open_diagnostics(cx) {
        second_view.update(cx, |this, cx| this.start(cx));
        cx.spawn(async move |cx| {
            cx.background_executor()
                .timer(std::time::Duration::from_millis(350))
                .await;
            let result = second.update(cx, |_, window, _| window.remove_window());
            eprintln!("SMOKE second_window_closed={}", result.is_ok());
        })
        .detach();
    }
    eprintln!("SMOKE native_adapter_sequence_dispatched");
}
fn main() {
    if std::env::args().any(|x| x == "--recovery-crash-probe") {
        let root = std::env::args()
            .skip_while(|x| x != "--recovery-crash-probe")
            .nth(1)
            .expect("test root");
        let mut s = joydeck::edit::Session::new();
        s.execute(
            s.id,
            joydeck::edit::Command::Batch(vec![
                joydeck::edit::Command::AddPage,
                joydeck::edit::Command::AddPage,
            ]),
        )
        .unwrap();
        let service = joydeck::storage::Recovery::new(root.into());
        service.write(1, &s.deck, None, None).unwrap();
        std::process::exit(73);
    }

    if std::env::args().any(|x| x == "--version") {
        println!(
            "JoyDeck {} · Kit 0.8.1 · GPUI pre 0.3.10",
            env!("CARGO_PKG_VERSION")
        );
        return;
    }
    std::panic::set_hook(Box::new(|info| {
        eprintln!("JoyDeck startup/runtime failure: {info}");
    }));
    let requests = Arc::new(std::sync::Mutex::new(
        std::collections::VecDeque::<String>::new(),
    ));
    let incoming = requests.clone();
    let application = gpui_kit::application().with_assets(gpui_kit::assets::AllAssets);
    application.on_open_urls(move |urls| {
        let mut queue = incoming.lock().unwrap();
        for url in urls.into_iter().take(32) {
            if queue.len() < 32 {
                queue.push_back(url);
            }
        }
    });
    application.on_reopen(|cx| {
        if let Some(window) = cx.windows().first() {
            let _ = window.update(cx, |_, w, _| w.activate_window());
        }
    });
    application.run(move |cx| {
        gpui_kit::init(cx);
        viewer::bind(cx);
        cx.set_menus([
            Menu::new("JoyDeck").items([
                MenuItem::os_submenu("服务", SystemMenuType::Services),
                MenuItem::separator(),
                MenuItem::action("退出 JoyDeck", Quit),
            ]),
            Menu::new("文件").items([
                MenuItem::action("新建窗口", NewWindow),
                MenuItem::action("打开…", OpenDocument),
                MenuItem::action("保存", editor::Save),
                MenuItem::action("另存为…", editor::SaveAs),
                MenuItem::action("关闭", editor::Close),
            ]),
            Menu::new("编辑").items([
                MenuItem::action("撤销", UndoFocused),
                MenuItem::action("重做", RedoFocused),
            ]),
        ]);
        cx.on_action(|_: &NewWindow, cx| {
            let _ = editor::open(cx);
        });
        cx.on_action(|_: &UndoFocused, cx| {
            if let Some(window) = cx.active_window() {
                // Menu actions bubble while the window is borrowed by GPUI. Route the
                // focused keystroke only after that dispatch has released the window.
                cx.defer(move |cx| {
                    let _ = window.update(cx, |_, w, cx| {
                        w.dispatch_keystroke(
                            Keystroke::parse(if cfg!(target_os = "macos") {
                                "cmd-z"
                            } else {
                                "ctrl-z"
                            })
                            .unwrap(),
                            cx,
                        );
                    });
                });
            }
        });
        cx.on_action(|_: &RedoFocused, cx| {
            if let Some(window) = cx.active_window() {
                cx.defer(move |cx| {
                    let _ = window.update(cx, |_, w, cx| {
                        w.dispatch_keystroke(
                            Keystroke::parse(if cfg!(target_os = "macos") {
                                "cmd-shift-z"
                            } else {
                                "ctrl-y"
                            })
                            .unwrap(),
                            cx,
                        );
                    });
                });
            }
        });
        cx.on_action(|_: &OpenDocument, cx| {
            let result = cx.prompt_for_paths(PathPromptOptions {
                files: true,
                directories: false,
                multiple: true,
                prompt: Some("打开演示文稿".into()),
            });
            cx.spawn(async move |cx| {
                if let Ok(Ok(Some(paths))) = result.await {
                    cx.update(|cx| {
                        for path in paths.into_iter().take(16) {
                            if let Err(e) = editor::open_path(path, cx) {
                                eprintln!("打开失败：{e}");
                            }
                        }
                    });
                }
            })
            .detach();
        });
        cx.spawn(async move |cx| {
            loop {
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(100))
                    .await;
                let batch = requests.lock().unwrap().drain(..).collect::<Vec<_>>();
                for url in batch {
                    cx.update(|cx| match joydeck::preferences::local_file_url(&url) {
                        Ok(path) => match editor::open_path(path, cx) {
                            Ok(()) => eprintln!("M13_OPEN_FILE accepted=true"),
                            Err(e) => eprintln!("打开失败：{e}"),
                        },
                        Err(e) => eprintln!("文件事件被拒绝：{e}"),
                    });
                }
            }
        })
        .detach();
        if std::env::args().any(|a| a == "--m4-bench") {
            editor::open_benchmark(cx);
            return;
        }
        Theme::change(ThemeMode::Light, None, cx);
        cx.on_action(|_: &Quit, cx| editor::quit(cx));
        cx.bind_keys([
            KeyBinding::new("ctrl-q", Quit, None),
            KeyBinding::new("ctrl-shift-t", ToggleTheme, Some("Diagnostics")),
            KeyBinding::new("ctrl-shift-l", ToggleLanguage, Some("Diagnostics")),
            KeyBinding::new("ctrl-shift-n", SecondWindow, Some("Diagnostics")),
        ]);
        cx.on_window_closed(|cx, _| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();
        if !std::env::args().any(|x| {
            x == "--diagnostics" || x == "--smoke" || x == "--viewer-smoke" || x == "--viewer"
        }) && !std::env::args().skip(1).any(|x| !x.starts_with("--"))
        {
            if let Err(e) = editor::open(cx) {
                eprintln!("Editor startup failed: {e}");
                cx.quit();
            }
            cx.activate(true);
            return;
        }
        if !std::env::args().any(|x| x == "--diagnostics" || x == "--smoke") {
            match viewer::open_window(cx) {
                Ok((handle, view)) => {
                    cx.activate(true);
                    eprintln!("JoyDeck M1 native viewer opened");
                    if std::env::args().any(|x| x == "--viewer-smoke") {
                        viewer::smoke(handle, &view, cx);
                    } else if let Some(path) =
                        std::env::args().skip(1).find(|x| !x.starts_with("--"))
                    {
                        viewer::load_path(&view, path.into(), cx);
                    }
                }
                Err(error) => {
                    eprintln!("JoyDeck viewer failed: {error}");
                    cx.quit();
                }
            }
            return;
        }
        match open_diagnostics(cx) {
            Ok((handle, view)) => {
                cx.activate(true);
                eprintln!("JoyDeck native window opened · Kit 0.8.1 / pre 0.3.10");
                if std::env::args().any(|x| x == "--smoke") {
                    cx.defer(move |cx| {
                        if let Err(error) =
                            handle.update(cx, |_, window, cx| smoke(window, &view, cx))
                        {
                            eprintln!("SMOKE failed: {error}");
                        }
                    });
                }
            }
            Err(error) => {
                eprintln!("JoyDeck failed to open: {error}");
                cx.quit();
            }
        }
    });
}
