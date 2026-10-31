//! GPUI desktop shell for Joysheet.
#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]
extern crate gpui_kit as gpui;
mod accessibility;
mod advanced;
mod automation;
mod cf_icons;
mod chart_dash;
mod charts;
mod complex_format;
mod control;
mod daily;
mod desktop;
mod formula_feedback;
mod grid;
mod grid_tools;
mod input;
mod object_tools;
mod objects;
mod outline_text;
mod palette;
mod performance;
mod printing;
mod recovery;
mod ribbon;
mod settings;
mod validation;
mod window_tools;
mod windows_quality;
mod workflow;

use gpui_kit::*;
use joysheet_app::EngineClient;
actions!(joysheet, [NewWorkbook, OpenWorkbook, SaveWorkbook, SaveAsWorkbook, CloseWorkbook, UndoWorkbook, RedoWorkbook, ShowCommands]);
pub struct SharedPreferences(pub joysheet_platform::Preferences);
impl Global for SharedPreferences {}
pub struct OpenRequests(pub std::sync::Mutex<std::sync::mpsc::Receiver<Vec<String>>>);
impl Global for OpenRequests {}
#[derive(Action, Clone, PartialEq, Eq, serde::Deserialize)]
#[action(no_json)]
pub struct WorksheetKey {
    pub key: String,
    pub shift: bool,
    pub command: bool,
}
fn install_worksheet_keys(cx: &mut App) {
    cx.bind_keys([KeyBinding::new("space", WorksheetKey { key: "space".into(), shift: false, command: false }, Some("JoysheetCheckbox"))]);
    let grid = [
        ("shift-space", "space", true, false),
        ("left", "left", false, false),
        ("right", "right", false, false),
        ("up", "up", false, false),
        ("down", "down", false, false),
        ("shift-left", "left", true, false),
        ("shift-right", "right", true, false),
        ("shift-up", "up", true, false),
        ("shift-down", "down", true, false),
        ("pageup", "pageup", false, false),
        ("pagedown", "pagedown", false, false),
        ("home", "home", false, false),
        ("end", "end", false, false),
        ("ctrl-home", "home", false, true),
        ("ctrl-end", "end", false, true),
        ("ctrl-left", "left", false, true),
        ("ctrl-right", "right", false, true),
        ("ctrl-up", "up", false, true),
        ("ctrl-down", "down", false, true),
        ("ctrl-shift-left", "left", true, true),
        ("ctrl-shift-right", "right", true, true),
        ("ctrl-shift-up", "up", true, true),
        ("ctrl-shift-down", "down", true, true),
        ("backspace", "backspace", false, false),
        ("delete", "delete", false, false),
        ("f2", "f2", false, false),
    ];
    for (chord, key, shift, command) in grid {
        cx.bind_keys([KeyBinding::new(chord, WorksheetKey { key: key.into(), shift, command }, Some("JoysheetGrid"))]);
    }
    for context in ["JoysheetGrid", "JoysheetEditor"] {
        for (chord, key, shift) in [
            ("tab", "tab", false),
            ("shift-tab", "tab", true),
            ("enter", "enter", false),
            ("shift-enter", "enter", true),
            ("escape", "escape", false),
        ] {
            cx.bind_keys([KeyBinding::new(chord, WorksheetKey { key: key.into(), shift, command: false }, Some(context))]);
        }
    }
}

#[derive(Clone, Default)]
pub struct LaunchOptions {
    pub sample: Option<String>,
    pub path: Option<String>,
    pub control: Option<u16>,
    pub diagnostics: bool,
}

pub fn run(options: LaunchOptions) -> Result<(), String> {
    outline_text::start();
    let engine = EngineClient::start()?;
    let initial = if let Some(path) = options.path {
        engine.execute("file.open", serde_json::json!({"path":path}))?
    } else {
        engine.execute("file.new", options.sample.map(|sample| serde_json::json!({"sample":sample})).unwrap_or_else(|| serde_json::json!({})))?
    };
    initial.result?;
    eprintln!("Joysheet: engine initialized");
    let server = options.control.map(|port| joysheet_app::control::ControlServer::start(port, engine.clone())).transpose()?;
    let recovery_service = std::sync::Arc::new(joysheet_app::recovery::RecoveryService::start(joysheet_platform::config_dir().join("recovery"))?);
    let recovery_entries = recovery_service.list()?;
    let diagnostics = options.diagnostics;
    let (opens, open_requests) = std::sync::mpsc::channel();
    let application = gpui_kit::application();
    application.on_open_urls(move |urls| {
        let _ = opens.send(urls);
    });
    application.run(move |cx| {
        eprintln!("Joysheet: initializing GPUI components");
        gpui_kit::init(cx);
        install_worksheet_keys(cx);
        install_text_measurements(&engine, cx);
        eprintln!("Joysheet: opening desktop window");
        let preferences = joysheet_platform::Preferences::load();
        cx.set_global(recovery::RecoveryRuntime {
            service: recovery_service,
            entries: recovery_entries,
            drafts: Default::default(),
            pending: Default::default(),
            last: std::time::Instant::now(),
        });
        cx.set_global(SharedPreferences(preferences.clone()));
        cx.set_global(OpenRequests(std::sync::Mutex::new(open_requests)));
        cx.set_menus([
            Menu::new("File").items([
                MenuItem::action("New", NewWorkbook),
                MenuItem::action("Open…", OpenWorkbook),
                MenuItem::action("Save", SaveWorkbook),
                MenuItem::action("Save As…", SaveAsWorkbook),
                MenuItem::separator(),
                MenuItem::action("Close", CloseWorkbook),
            ]),
            Menu::new("Edit").items([
                MenuItem::action("Undo", UndoWorkbook),
                MenuItem::action("Redo", RedoWorkbook),
                MenuItem::action("Commands…", ShowCommands),
            ]),
        ]);
        gpui_kit::component::Theme::change(
            if preferences.dark { gpui_kit::component::ThemeMode::Dark } else { gpui_kit::component::ThemeMode::Light },
            None,
            cx,
        );
        cx.set_global(desktop::NativeWindows::default());
        cx.set_global(desktop::NativeFocus::default());
        let queue = server.as_ref().map(|s| s.queue.clone()).unwrap_or_default();
        cx.set_global(control::ControlRuntime { _server: server, queue, diagnostics });
        let id = engine.latest().active_window;
        if let Err(error) = desktop::open(engine, id, cx) {
            eprintln!("Joysheet: {error}");
            cx.quit();
        }
        cx.activate(true);
        eprintln!("Joysheet: desktop window initialized");
    });
    Ok(())
}

fn install_text_measurements(engine: &EngineClient, cx: &App) {
    let shared = cx.text_system().clone();
    let meter = std::sync::Mutex::new((WindowTextSystem::new(shared.clone()), 0usize));
    let measure: joysheet_engine::TextWidth = std::sync::Arc::new(move |text, font, _exact| {
        let mut meter = meter.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        if meter.1 >= 4096 {
            meter.0 = WindowTextSystem::new(shared.clone());
            meter.1 = 0;
        }
        let font_size = font.size * 4. / 3.;
        let font = Font {
            family: font.name.clone().into(),
            weight: if font.bold { FontWeight::BOLD } else { FontWeight::NORMAL },
            style: if font.italic { FontStyle::Italic } else { FontStyle::Normal },
            ..Default::default()
        };
        let mut width = 0f32;
        for line in text.lines().take(1024) {
            let run = TextRun { len: line.len(), font: font.clone(), ..Default::default() };
            width = width.max(f32::from(meter.0.shape_line(line.to_string().into(), px(font_size), &[run], None).width()));
            meter.1 += 1;
        }
        width
    });
    if let Err(error) = engine.install_text_width(measure) {
        eprintln!("Joysheet text measurement: {error}");
    }
}
