use gpui_kit::component::input::{Input, InputEvent as TextEvent, InputState, TextareaState};
use gpui_kit::{prelude::*, *};
use gridcraft_engine::{Selection, UiRequest, core::CellRef};
use joysheet_app::{
    AppSnapshot, EngineClient, Ticket, WindowSnapshot,
    geometry::{GridLayout, SheetGeometry},
};
use joysheet_platform::{Clipboard, Preferences};
use joysheet_protocol::CommandTarget;
use serde_json::{Value, json};
use std::{collections::HashMap, sync::Arc, time::Duration};

#[derive(Default)]
pub struct NativeWindows(pub HashMap<u64, AnyWindowHandle>, pub std::collections::HashSet<u64>, pub HashMap<u64, u64>);
impl Global for NativeWindows {}
/// Engine activation is an intent until the corresponding native window acknowledges it.
/// Closing a window can transiently activate another HWND before the replacement opens.
#[derive(Default)]
pub struct NativeFocus {
    pub sequence: u64,
    pub active: Option<u64>,
    pub waiting: Option<u64>,
}
impl Global for NativeFocus {}
impl NativeFocus {
    pub fn observe(&mut self, snapshot: &AppSnapshot) {
        if snapshot.sequence < self.sequence {
            return;
        }
        self.sequence = snapshot.sequence;
        if self.active != snapshot.active_window {
            self.active = snapshot.active_window;
            self.waiting = snapshot.active_window;
        }
    }
}
pub fn open(engine: EngineClient, id: Option<u64>, cx: &mut App) -> Result<(), String> {
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds::centered(None, size(px(1440.0), px(900.0)), cx))),
        titlebar: Some(TitlebarOptions { title: Some("Joysheet".into()), ..Default::default() }),
        ..Default::default()
    };
    let (handle, _) =
        gpui_kit::open_window(options, cx, |window, cx| cx.new(|cx| Desktop::new(engine, id, window, cx))).map_err(|e| e.to_string())?;
    if let Some(id) = id {
        let native = handle
            .update(cx, |_, window, _| {
                raw_window_handle::HasWindowHandle::window_handle(window).ok().and_then(|h| match h.as_raw() {
                    raw_window_handle::RawWindowHandle::Win32(w) => Some(w.hwnd.get() as u64),
                    _ => None,
                })
            })
            .ok()
            .flatten();
        cx.update_global::<NativeWindows, _>(|windows, _| {
            windows.0.insert(id, handle);
            if let Some(native) = native {
                windows.2.insert(id, native);
            }
        });
    }
    Ok(())
}

#[derive(Clone)]
pub struct Draft {
    pub window: u64,
    pub doc: u64,
    pub sheet: usize,
    pub sheet_name: String,
    pub cell: CellRef,
    pub original: String,
    pub text: String,
    pub inline: bool,
}
#[derive(Clone, Default)]
pub enum After {
    #[default]
    None,
    Move(i64, i64),
    Select(CellRef, bool),
    Command(String, Value),
    Control(u64, String, Value),
    Save(bool),
    Open,
    OpenPaths(Vec<String>),
    Close(bool),
    CloseSaved,
    Recovered(Value),
    RestoreDraft(Value),
}
pub struct Pending {
    pub ticket: Ticket,
    pub after: After,
    pub commit: bool,
    pub clipboard: bool,
}
pub struct ReferencePick {
    pub anchor: CellRef,
    pub range: std::ops::Range<usize>,
    pub inline: bool,
    pub text: String,
}

pub struct Desktop {
    pub performance: crate::performance::Performance,
    pub ime_geometry: Option<(Bounds<Pixels>, f32)>,
    pub last_arrangement: Value,
    pub focus_band: u8,
    pub sheets_focus: FocusHandle,
    pub ribbon_focus: FocusHandle,
    pub key_trace: Vec<String>,
    pub outline_fonts_ready: bool,
    pub engine: EngineClient,
    pub snapshot: Arc<AppSnapshot>,
    pub window_id: Option<u64>,
    pub pending: Vec<Pending>,
    pub formula: Entity<InputState>,
    pub editor: Entity<InputState>,
    pub name: Entity<InputState>,
    pub search: Entity<InputState>,
    pub params: Entity<InputState>,
    pub import_text: Entity<TextareaState>,
    pub import_draft: Option<Value>,
    pub draft: Option<Draft>,
    pub committing: bool,
    pub preferences: Preferences,
    pub clipboard: Clipboard,
    pub message: String,
    pub error: bool,
    pub error_count: u64,
    pub barriers: Vec<crate::control::Barrier>,
    pub next_barrier: u64,
    pub scroll: (f64, f64),
    pub sheet_views: HashMap<(u64, String), ((f64, f64), CellRef)>,
    pub geometry: Option<SheetGeometry>,
    pub geometry_key: Option<(u64, usize, u64)>,
    pub cf_cache: std::rc::Rc<std::cell::RefCell<gridcraft_engine::cf::CfCache>>,
    pub chart_cache: crate::charts::ChartCache,
    pub object_images: crate::objects::Images,
    pub image_decoding: usize,
    pub selected_object: Option<crate::objects::ObjectKey>,
    pub selected_objects: Vec<crate::objects::ObjectKey>,
    pub object_drag: Option<crate::objects::ObjectDrag>,
    pub print_jobs: HashMap<u64, crate::printing::PrintJob>,
    pub print_generation: u64,
    pub print_dialog_generation: Option<u64>,
    pub print_rendering: usize,
    pub preview: Option<crate::printing::Preview>,
    pub print_status: String,
    pub layout: GridLayout,
    pub grid_bounds: Bounds<Pixels>,
    pub local_selection: Option<Selection>,
    pub drag_anchor: Option<CellRef>,
    pub reference_pick: Option<ReferencePick>,
    pub bar_drag: Option<crate::grid_tools::ScrollDrag>,
    pub resize_drag: Option<crate::grid_tools::DimensionDrag>,
    pub edge_drag: Option<(f32, f32)>,
    pub sheet_drag: Option<(usize, u64)>,
    pub palette_open: bool,
    pub selected_command: Option<String>,
    pub command_fields: Vec<crate::palette::CommandField>,
    pub advanced_params: bool,
    pub recent_open: bool,
    pub daily_group: u8,
    pub ribbon_tab: usize,
    pub data_results: Vec<(String, Option<String>)>,
    pub data_result_kind: String,
    pub data_raw: Value,
    pub script_bodies: Value,
    pub script_doc: Option<u64>,
    pub validation_prompt: Option<crate::validation::ValidationPrompt>,
    pub validation_accepted: bool,
    pub trust_request: Option<crate::automation::TrustRequest>,
    pub trust_focus: FocusHandle,
    pub pivot_drag: Option<(String, u64, Value)>,
    pub advanced_bounds: HashMap<String, Bounds<Pixels>>,
    pub result_requests: HashMap<u64, String>,
    pub confirm_close: bool,
    pub close_saving: bool,
    pub close_focus: FocusHandle,
    pub close_button_bounds: HashMap<&'static str, Bounds<Pixels>>,
    pub allow_close: bool,
    pub painted_cells: usize,
    pub paint_ms: f64,
    pub frame_samples: Vec<f64>,
    pub edit_clock: Option<(std::time::Instant, u64)>,
    pub edit_fence: Option<u64>,
    pub edit_samples: Vec<f64>,
    pub last_edit: std::time::Instant,
    pub autosave_revision: Option<u64>,
    pub frame_number: u64,
    pub painted_sequence: u64,
    pub captures: Vec<crate::control::Capture>,
    pub _subscriptions: Vec<Subscription>,
}

impl Desktop {
    pub fn new(engine: EngineClient, id: Option<u64>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let formula = cx.new(|cx| InputState::new(window, cx).placeholder("输入数据或公式 / Enter a value or formula"));
        let editor = cx.new(|cx| InputState::new(window, cx));
        let name = cx.new(|cx| InputState::new(window, cx).default_value("A1"));
        let search = cx.new(|cx| InputState::new(window, cx).placeholder("搜索命令 / Search commands"));
        let params = cx.new(|cx| InputState::new(window, cx).default_value("{}"));
        let import_text = cx.new(|cx| TextareaState::new(window, cx));
        let mut subscriptions = Vec::new();
        subscriptions.push(cx.observe_window_activation(window, |this, window, cx| {
            if !window.is_window_active() {
                return;
            }
            let latest = this.engine.latest();
            let accept = cx.update_global::<NativeFocus, _>(|focus, _| {
                focus.observe(&latest);
                if latest.active_window == this.window_id {
                    focus.waiting = None;
                    false
                } else {
                    focus.waiting.is_none() && this.snapshot.active_window == latest.active_window
                }
            });
            if accept && let Some(id) = this.window_id {
                this.issue("__joysheet.nativeActivate", json!({"window":id,"previousActive":latest.active_window}), After::None, cx);
            }
        }));
        subscriptions.push(cx.subscribe_in(&editor, window, |this, input, event, window, cx| {
            if matches!(event, TextEvent::Change) {
                let text = input.read(cx).value().to_string();
                this.input_changed(text, true, window, cx);
            }
        }));
        subscriptions.push(cx.subscribe_in(&formula, window, |this, input, event, window, cx| match event {
            TextEvent::Change => {
                let text = input.read(cx).value().to_string();
                this.input_changed(text, false, window, cx);
            }
            TextEvent::Focus => this.begin_formula(window, cx),
            _ => {}
        }));
        // GPUI's input engine deliberately does not emit Change for IME preedit.
        // Observe marked text so the shared draft and visible editor follow composition too.
        for (input, inline) in [(editor.clone(), true), (formula.clone(), false)] {
            subscriptions.push(cx.observe_in(&input, window, move |this, input, window, cx| {
                let marked = input.update(cx, |input, cx| input.marked_text_range(window, cx).is_some());
                let text = input.read(cx).value().to_string();
                if (marked || this.draft.as_ref().is_some_and(|d| d.inline == inline)) && this.draft.as_ref().is_none_or(|d| d.text != text) {
                    this.input_changed(text, inline, window, cx);
                }
                if this.draft.as_ref().is_some_and(|d| d.inline == inline) {
                    cx.notify();
                }
            }));
        }
        subscriptions.push(cx.subscribe_in(&name, window, |this, input, event, window, cx| {
            if matches!(event, TextEvent::PressEnter { .. }) {
                let reference = input.read(cx).value().to_string();
                this.commit(After::Command("edit.goTo".into(), json!({"reference":reference})), window, cx);
                this.focus_grid(window, cx);
            }
        }));
        subscriptions.push(cx.subscribe(&search, |_, _, _: &TextEvent, cx| cx.notify()));
        let entity = cx.entity().downgrade();
        window.on_window_should_close(cx, move |window, cx| {
            entity
                .update(cx, |this, cx| {
                    if this.allow_close {
                        return true;
                    }
                    this.commit(After::Close(false), window, cx);
                    false
                })
                .unwrap_or(true)
        });
        let updates = engine.subscribe();
        cx.spawn_in(window, async move |this, cx| {
            while updates.recv().await.is_ok() {
                if this.update_in(cx, |this, window, cx| this.poll(window, cx)).is_err() {
                    break;
                }
            }
        })
        .detach();
        // Keep the timer for control requests, pointer autoscroll and recovery checks.
        cx.spawn_in(window, async move |this, cx| {
            loop {
                cx.background_executor().timer(Duration::from_millis(16)).await;
                if this.update_in(cx, |this, window, cx| this.poll(window, cx)).is_err() {
                    break;
                }
            }
        })
        .detach();
        let snapshot = engine.latest();
        let mut this = Self {
            performance: Default::default(),
            ime_geometry: None,
            last_arrangement: Value::Null,
            focus_band: 0,
            sheets_focus: cx.focus_handle(),
            ribbon_focus: cx.focus_handle(),
            key_trace: Vec::new(),
            outline_fonts_ready: false,
            engine,
            snapshot,
            window_id: id,
            pending: Vec::new(),
            formula,
            editor,
            name,
            search,
            params,
            import_text,
            import_draft: None,
            draft: None,
            committing: false,
            preferences: cx.global::<crate::SharedPreferences>().0.clone(),
            clipboard: Clipboard::default(),
            message: "就绪 · Ready".into(),
            error: false,
            error_count: 0,
            barriers: Vec::new(),
            next_barrier: 0,
            scroll: (0.0, 0.0),
            sheet_views: HashMap::new(),
            geometry: None,
            geometry_key: None,
            cf_cache: Default::default(),
            chart_cache: Default::default(),
            object_images: Default::default(),
            image_decoding: 0,
            selected_object: None,
            selected_objects: Vec::new(),
            object_drag: None,
            print_jobs: HashMap::new(),
            print_generation: 0,
            print_dialog_generation: None,
            print_rendering: 0,
            preview: None,
            print_status: String::new(),
            layout: GridLayout::default(),
            grid_bounds: Bounds::default(),
            local_selection: None,
            drag_anchor: None,
            reference_pick: None,
            bar_drag: None,
            resize_drag: None,
            edge_drag: None,
            sheet_drag: None,
            palette_open: false,
            selected_command: None,
            command_fields: Vec::new(),
            advanced_params: false,
            recent_open: false,
            ribbon_tab: 1,
            daily_group: if cx.global::<NativeWindows>().0.is_empty() && !cx.global::<crate::recovery::RecoveryRuntime>().entries.is_empty() {
                21
            } else {
                0
            },
            data_results: Vec::new(),
            data_result_kind: String::new(),
            data_raw: Value::Null,
            script_bodies: json!({}),
            script_doc: None,
            validation_prompt: None,
            validation_accepted: false,
            trust_request: None,
            trust_focus: cx.focus_handle(),
            pivot_drag: None,
            advanced_bounds: HashMap::new(),
            result_requests: HashMap::new(),
            confirm_close: false,
            close_saving: false,
            close_focus: cx.focus_handle(),
            close_button_bounds: HashMap::new(),
            allow_close: false,
            painted_cells: 0,
            paint_ms: 0.0,
            frame_samples: Vec::new(),
            edit_clock: None,
            edit_fence: None,
            edit_samples: Vec::new(),
            last_edit: std::time::Instant::now(),
            autosave_revision: None,
            frame_number: 0,
            painted_sequence: 0,
            captures: Vec::new(),
            _subscriptions: subscriptions,
        };
        this.sync_inputs(window, cx);
        if let Some(path) = this.view().and_then(|v| v.path.clone()) {
            this.preferences.remember(path);
            this.persist_preferences(cx);
        }
        this.focus_grid(window, cx);
        this
    }
    pub fn view(&self) -> Option<&WindowSnapshot> {
        self.window_id.and_then(|id| self.snapshot.window(id)).or_else(|| self.snapshot.active())
    }
    pub fn target(&self) -> CommandTarget {
        CommandTarget { window: self.window_id, doc: None }
    }
    pub fn selection(&self) -> Selection {
        self.local_selection.clone().or_else(|| self.view().map(|v| v.selection.clone())).unwrap_or_default()
    }
    pub fn chinese(&self) -> bool {
        self.preferences.language == "zh"
    }
    pub fn label(&self, zh: &str, en: &str) -> String {
        if self.chinese() { zh.into() } else { en.into() }
    }
    pub fn issue(&mut self, command: &str, params: Value, after: After, cx: &mut Context<Self>) {
        if command == "automate.runScript" {
            self.daily_group = 16;
        }
        if matches!(command, "file.printPreview" | "file.print") {
            self.begin_print(params, command == "file.print", cx);
            return;
        }
        let target = if matches!(command, "window.activate" | "__joysheet.nativeActivate") { CommandTarget::default() } else { self.target() };
        match self.engine.submit(command, params, target) {
            Ok(ticket) => {
                if crate::advanced::is_result(command)
                    || ["edit.find", "edit.replace", "data.queriesConnections", "data.circleInvalid", "home.manageRules", "__joysheet.previewImport"]
                        .contains(&command)
                {
                    self.result_requests.insert(ticket.id, command.into());
                }
                self.pending.push(Pending { ticket, after, commit: false, clipboard: matches!(command, "edit.copy" | "edit.cut") });
                self.error = false;
            }
            Err(error) => self.alert(error),
        }
        cx.notify();
    }
    pub fn alert(&mut self, error: impl Into<String>) {
        self.error_count = self.error_count.saturating_add(1);
        self.message = error.into();
        self.error = true;
    }
    pub fn persist_preferences(&mut self, cx: &mut Context<Self>) {
        cx.set_global(crate::SharedPreferences(self.preferences.clone()));
        if let Err(error) = self.preferences.save() {
            self.alert(error);
        }
    }
    pub fn focus_grid(&self, window: &mut Window, cx: &mut Context<Self>) {
        self.editor.update(cx, |input, cx| input.focus(window, cx));
    }
    pub fn sync_inputs(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.draft.is_some()
            || self.editor.update(cx, |input, cx| input.marked_text_range(window, cx).is_some())
            || self.formula.update(cx, |input, cx| input.marked_text_range(window, cx).is_some())
        {
            return;
        }
        let (address, text) = self
            .view()
            .and_then(|v| v.sheet().map(|s| (v.selection.active.a1(), gridcraft_engine::display::edit_text(&v.workbook, s, v.selection.active))))
            .unwrap_or_default();
        self.formula.update(cx, |input, cx| {
            if input.value().as_ref() != text {
                input.set_value(text, window, cx);
            }
        });
        self.editor.update(cx, |input, cx| {
            if !input.value().is_empty() {
                input.set_value("", window, cx);
            }
        });
        self.name.update(cx, |input, cx| {
            if !input.focus_handle(cx).is_focused(window) && input.value().as_ref() != address {
                input.set_value(address, window, cx);
            }
        });
    }
    fn poll(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.outline_fonts_ready && crate::outline_text::ready() {
            self.outline_fonts_ready = true;
            cx.notify();
        }
        self.auto_scroll_drag(cx);
        self.recovery_tick(window, cx);
        if self.window_id == self.snapshot.active_window {
            let urls = cx.global::<crate::OpenRequests>().0.lock().unwrap_or_else(std::sync::PoisonError::into_inner).try_recv().ok();
            if let Some(urls) = urls {
                let paths = urls
                    .iter()
                    .map(|u| joysheet_platform::local_open_path(u))
                    .collect::<Result<Vec<_>, _>>()
                    .and_then(joysheet_platform::workbook_paths);
                match paths {
                    Ok(paths) => self.commit(After::OpenPaths(paths), window, cx),
                    Err(e) => self.alert(e),
                }
            }
        }
        let shared = &cx.global::<crate::SharedPreferences>().0;
        if &self.preferences != shared {
            self.preferences = shared.clone();
            cx.notify();
        }
        // The first layout establishes the active cell editor's overlay bounds.
        // Request its follow-up frame after paint, not from inside prepaint.
        if self.frame_number < 2 {
            cx.notify();
        }
        let mut ready = Vec::new();
        let mut i = 0;
        while i < self.pending.len() {
            if let Some(result) = self.pending.get(i).and_then(|p| p.ticket.poll()) {
                if let Ok(c) = &result {
                    self.performance.completed(c, std::time::Instant::now());
                }
                ready.push((self.pending.remove(i), result));
            } else {
                i += 1;
            }
        }
        let latest = self.engine.latest();
        let wants_focus = cx.update_global::<NativeFocus, _>(|focus, _| {
            focus.observe(&latest);
            if focus.waiting == self.window_id && self.window_id.is_some() {
                if window.is_window_active() {
                    focus.waiting = None;
                } else {
                    return true;
                }
            }
            false
        });
        if wants_focus {
            window.activate_window();
        }
        let changed = latest.sequence != self.snapshot.sequence;
        let mut reveal = false;
        if changed {
            let before = self.view().map(|v| (v.doc, v.revision));
            let prior_path = self.view().and_then(|v| v.path.clone());
            let prior_location = self.view().map(|v| (v.doc, v.sheet, v.selection.active));
            let prior_sheet = self.view().and_then(|v| v.sheet().map(|s| (v.doc, s.name.clone(), v.selection.active)));
            let prior_active = self.snapshot.active_window;
            let closed: std::collections::HashSet<_> =
                self.snapshot.windows.iter().map(|w| w.doc).filter(|id| !latest.windows.iter().any(|w| w.doc == *id)).collect();
            for doc in closed {
                if let Err(e) = cx.global::<crate::recovery::RecoveryRuntime>().service.forget(doc) {
                    self.alert(e);
                }
            }
            cx.update_global::<crate::recovery::RecoveryRuntime, _>(|r, _| {
                r.drafts.retain(|window, _| latest.windows.iter().any(|w| w.window == *window))
            });
            self.snapshot = latest;
            let next_sheet = self.view().and_then(|v| v.sheet().map(|s| (v.doc, s.name.clone(), v.selection.active)));
            let mut restored = false;
            if prior_sheet.as_ref().map(|v| (v.0, &v.1)) != next_sheet.as_ref().map(|v| (v.0, &v.1)) {
                if let Some((doc, name, cell)) = prior_sheet {
                    self.sheet_views.insert((doc, name), (self.scroll, cell));
                }
                self.scroll = (0., 0.);
                if let Some((doc, name, cell)) = next_sheet
                    && let Some((scroll, selected)) = self.sheet_views.get(&(doc, name))
                {
                    self.scroll = *scroll;
                    restored = *selected == cell;
                }
            }
            if prior_active != self.snapshot.active_window && self.snapshot.active_window == self.window_id {
                window.activate_window();
            }
            if let Some(path) = self.view().and_then(|v| v.path.clone()).filter(|path| Some(path) != prior_path.as_ref()) {
                self.preferences.remember(path);
                self.persist_preferences(cx);
            }
            reveal = !restored && self.view().map(|v| (v.doc, v.sheet, v.selection.active)) != prior_location;
            if self.view().map(|v| (v.doc, v.revision)) != before {
                self.last_edit = std::time::Instant::now();
            }
            self.local_selection = None;
        }
        for (pending, result) in ready {
            if let After::Control(id, _, _) = &pending.after {
                let error = match &result {
                    Err(e) => Some(e.clone()),
                    Ok(c) => c.result.as_ref().err().cloned(),
                };
                if let Some(error) = error
                    && let Some(b) = self.barriers.iter_mut().find(|b| b.id == *id)
                {
                    b.result = Some(Err(error));
                }
            }
            if let Some(index) = self.barriers.iter().position(|b| b.ticket == Some(pending.ticket.id)) {
                let response = match &result {
                    Err(e) => Err(e.clone()),
                    Ok(c) => match &c.result {
                        Err(e) => Err(e.clone()),
                        Ok(value) => {
                            let kind = &self.barriers[index].kind;
                            let view = if kind == "app.save" {
                                self.window_id.and_then(|id| c.snapshot.window(id))
                            } else if kind == "app.open" && value.get("alreadyOpen").and_then(Value::as_bool) == Some(true) {
                                self.barriers[index]
                                    .call
                                    .params
                                    .get("path")
                                    .and_then(Value::as_str)
                                    .and_then(|path| c.snapshot.windows.iter().find(|w| w.path.as_deref() == Some(path)))
                            } else {
                                c.snapshot.active()
                            };
                            view.map(|v| {
                                let mut out = if value.is_object() { value.clone() } else { json!({"result":value}) };
                                out["window"] = json!(v.window);
                                out["doc"] = json!(v.doc);
                                out["path"] = json!(v.path);
                                out["dirty"] = json!(v.dirty);
                                out["revision"] = json!(v.revision);
                                out["sequence"] = json!(c.snapshot.sequence);
                                out["timing"] = json!(c.timing);
                                out
                            })
                            .ok_or_else(|| "No completed document".to_string())
                        }
                    },
                };
                self.barriers[index].result = Some(response);
            }
            if matches!(&pending.after, After::CloseSaved) {
                self.close_saving = false;
            }
            if let Some(job) = self.print_jobs.remove(&pending.ticket.id) {
                let result = result.and_then(|c| c.result);
                self.finish_print_export(job, result, window, cx);
                continue;
            }
            let result_kind = self.result_requests.remove(&pending.ticket.id);
            let has_data_result = result_kind.is_some();
            match result {
                Err(error) => {
                    if pending.commit {
                        self.committing = false;
                        self.edit_clock = None;
                        self.edit_fence = None;
                    }
                    self.alert(error);
                }
                Ok(completion) => {
                    match completion.result {
                        Err(error) => {
                            if pending.commit {
                                self.committing = false;
                                self.edit_clock = None;
                                self.edit_fence = None;
                            }
                            self.alert(error);
                        }
                        Ok(value) => {
                            if let Some(kind) = result_kind {
                                self.data_raw = value.clone();
                                self.data_results = crate::workflow::result_rows(&kind, &value);
                                self.data_result_kind = kind.clone();
                                self.daily_group = 8;
                                if kind == "pivot.fields" || kind == "pivot.fieldList" {
                                    self.daily_group = 18;
                                }
                                if kind.starts_with("review.") && !matches!(kind.as_str(), "review.workbookStatistics" | "review.checkAccessibility")
                                {
                                    self.daily_group = 15;
                                }
                                if kind == "formulas.nameManager" {
                                    self.daily_group = 19;
                                }
                                if kind.starts_with("automate.") || kind == "__joysheet.scriptDetails" {
                                    self.daily_group = 16;
                                }
                                if kind == "__joysheet.scriptDetails" {
                                    self.script_bodies = value.clone();
                                    self.script_doc = self.view().map(|v| v.doc);
                                }
                                if matches!(
                                    kind.as_str(),
                                    "automate.newScript" | "automate.deleteScript" | "automate.recordActions" | "automate.allScripts"
                                ) {
                                    self.issue("__joysheet.scriptDetails", json!({}), After::None, cx);
                                }
                                if kind.starts_with("pivot.") && !matches!(kind.as_str(), "pivot.fields" | "pivot.fieldList" | "pivot.list") {
                                    self.issue("pivot.fields", json!({}), After::None, cx);
                                }
                            }
                            self.error = false;
                            if pending.clipboard
                                && let Some(text) = value.get("text").and_then(Value::as_str)
                                && let Err(error) = self.clipboard.write(text, value.get("html").and_then(Value::as_str))
                            {
                                self.alert(error);
                            }
                            if pending.commit {
                                self.edit_fence = self.view().map(|v| v.revision);
                                self.draft = None;
                                self.committing = false;
                                self.track_draft(cx);
                            }
                            if let Some(path) = value.get("path").and_then(Value::as_str) {
                                self.preferences.remember(path.into());
                                self.persist_preferences(cx);
                            }
                            self.message = if has_data_result {
                                format!("{} {}", self.data_results.len(), self.label("项结果", "results"))
                            } else if value.is_null() {
                                "就绪 · Ready".into()
                            } else if cx.global::<crate::control::ControlRuntime>().diagnostics {
                                value.to_string().chars().take(180).collect()
                            } else {
                                self.label("操作完成", "Done")
                            };
                            self.after(pending.after, window, cx);
                            self.apply_window_arrangement(&value, window, cx);
                        }
                    }
                    for request in completion.requests {
                        self.ui_request(request, window, cx);
                    }
                }
            }
        }
        if changed {
            self.sync_inputs(window, cx);
            self.prepare_images(cx);
            if reveal {
                self.reveal_selection();
            }
            let title =
                self.view().map(|v| format!("{}{} — Joysheet", if v.dirty { "● " } else { "" }, v.title)).unwrap_or_else(|| "Joysheet".into());
            window.set_window_title(&title);
            let missing: Vec<u64> =
                self.snapshot.windows.iter().map(|w| w.window).filter(|id| !cx.global::<NativeWindows>().0.contains_key(id)).collect();
            for id in missing {
                if let Err(error) = open(self.engine.clone(), Some(id), cx) {
                    self.alert(error);
                }
            }
            if self.window_id.is_some_and(|id| self.snapshot.window(id).is_none()) {
                self.finish_barriers();
                for capture in self.captures.drain(..) {
                    capture.call.respond(Err("target window closed before capture".into()));
                }
                if let Some(id) = self.window_id {
                    cx.update_global::<NativeWindows, _>(|windows, _| {
                        windows.0.remove(&id);
                        windows.1.remove(&id);
                        windows.2.remove(&id);
                    });
                }
                self.allow_close = true;
                window.remove_window();
                if self.snapshot.windows.is_empty() {
                    cx.quit();
                }
                return;
            }
            cx.notify();
        }
        if self.preferences.autosave
            && !self.confirm_close
            && self.pending.is_empty()
            && self.draft.is_none()
            && self.last_edit.elapsed() > Duration::from_secs(2)
            && let Some(view) = self.view()
            && view.dirty
            && view.path.as_ref().is_some_and(|p| p.to_ascii_lowercase().ends_with(".xlsx"))
            && self.autosave_revision != Some(view.revision)
        {
            self.autosave_revision = Some(view.revision);
            self.issue("file.save", json!({}), After::None, cx);
        }
        self.refresh_ime_geometry(window, cx);
        self.performance.applied(self.snapshot.sequence);
        self.poll_control(window, cx);
        if self.daily_group == 16 && self.engine.tasks().iter().any(|t| matches!(t.state.as_str(), "running" | "cancelling")) {
            cx.notify();
        }
    }
    pub fn reveal_selection(&mut self) {
        if let (Some(view), Some(geo)) = (self.view(), self.geometry.as_ref()) {
            let freeze = view.sheet().and_then(|s| s.freeze).unwrap_or((0, 0));
            self.scroll = self.layout.reveal(view.selection.active, geo, freeze);
        }
    }
    pub fn ui_request(&mut self, request: UiRequest, window: &mut Window, cx: &mut Context<Self>) {
        match request {
            UiRequest::Message(message) => {
                self.message = message;
                cx.notify();
            }
            UiRequest::EditCell(text) => self.begin_edit(text, true, window, cx),
            UiRequest::Dialog(name, params) => {
                if name == "saveChanges" {
                    self.close(false, window, cx);
                    return;
                }
                if name == "options" {
                    self.daily_group = 20;
                    cx.notify();
                    return;
                }
                if name == "open" {
                    self.pick_open(window, cx);
                } else if name == "saveAs" {
                    self.pick_save(window, cx);
                } else {
                    self.open_command(&name, window, cx);
                    let mut values = serde_json::from_str::<Value>(&self.params.read(cx).value()).unwrap_or_else(|_| json!({}));
                    if let (Some(base), Some(extra)) = (values.as_object_mut(), params.as_object()) {
                        base.extend(extra.clone());
                    }
                    self.make_command_fields(&values, window, cx);
                    self.params.update(cx, |input, cx| input.set_value(values.to_string(), window, cx));
                    self.focus_command_input(window, cx);
                    cx.notify();
                }
            }
            UiRequest::InsertName(name) => self.insert_reference(&name, window, cx),
            UiRequest::OpenUrl(url) => {
                cx.open_url(&url);
            }
        }
    }
    pub fn after(&mut self, after: After, window: &mut Window, cx: &mut Context<Self>) {
        match after {
            After::None => {}
            After::Recovered(draft) => {
                if !draft.is_null()
                    && let Some(v) = self.engine.latest().active()
                {
                    let doc = v.doc;
                    cx.update_global::<crate::recovery::RecoveryRuntime, _>(|r, _| {
                        r.pending.insert(doc, draft);
                    });
                }
                self.message = "已恢复为独立工作簿；请另存为 / Recovered; use Save As".into();
            }
            After::RestoreDraft(draft) => {
                if let Some(cell) = draft["cell"].as_str().and_then(CellRef::parse) {
                    self.select(cell, false, cx);
                    self.begin_edit(draft["text"].as_str().map(String::from), true, window, cx);
                }
            }
            After::Move(dr, dc) => self.move_selection(dr, dc, false, false, cx),
            After::Select(cell, extend) => self.select(cell, extend, cx),
            After::Command(command, params) => self.issue(&command, params, After::None, cx),
            After::Control(id, command, params) => self.start_control_command(id, &command, params, cx),
            After::Save(save_as) => self.save(save_as, window, cx),
            After::Open => self.pick_open_now(window, cx),
            After::OpenPaths(paths) => {
                for path in paths {
                    self.issue("file.open", json!({"path":path}), After::None, cx);
                }
            }
            After::Close(force) => self.close(force, window, cx),
            After::CloseSaved => {
                if self.view().is_some_and(|v| v.dirty) {
                    self.alert("导出未保存完整工作簿，请保存为 XLSX / Export did not save the workbook; save as XLSX");
                    cx.notify();
                } else {
                    self.close(true, window, cx);
                }
            }
        }
    }
    pub fn select(&mut self, mut cell: CellRef, extend: bool, cx: &mut Context<Self>) {
        if let Some(merge) = self.view().and_then(|v| v.sheet()).and_then(|s| s.merge_at(cell)) {
            cell = merge.start;
        }
        let mut selection = if extend { self.selection() } else { Selection::at(cell) };
        if extend {
            selection.extend_to(cell);
        }
        self.local_selection = Some(selection.clone());
        self.issue("selection.set", json!({"range":selection.a1(),"active":cell.a1()}), After::None, cx);
    }
    pub fn move_selection(&mut self, dr: i64, dc: i64, extend: bool, jump: bool, cx: &mut Context<Self>) {
        self.issue("selection.move", json!({"dr":dr,"dc":dc,"extend":extend,"jump":jump}), After::None, cx);
    }
    pub fn pick_open(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.commit(After::Open, window, cx);
    }
    fn pick_open_now(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        cx.spawn_in(window, async move |this, cx| {
            if let Some(path) = joysheet_platform::pick_open().await {
                let _ = this.update_in(cx, |this, _, cx| this.issue("file.open", json!({"path":path}), After::None, cx));
            }
        })
        .detach();
    }
    pub fn pick_save(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let name = self.view().map(|v| v.title.clone()).unwrap_or_else(|| "Book1.xlsx".into());
        let name = if name.ends_with(".xlsx") { name } else { format!("{name}.xlsx") };
        cx.spawn_in(window, async move |this, cx| {
            if let Some(path) = joysheet_platform::pick_save(&name).await {
                let _ = this.update_in(cx, |this, _, cx| this.issue("file.save", json!({"path":path}), After::None, cx));
            }
        })
        .detach();
    }
    pub fn save(&mut self, save_as: bool, window: &mut Window, cx: &mut Context<Self>) {
        if save_as || self.view().is_none_or(|v| v.path.is_none()) {
            self.pick_save(window, cx);
        } else {
            self.issue("file.save", json!({}), After::None, cx);
        }
    }
    fn close(&mut self, force: bool, window: &mut Window, cx: &mut Context<Self>) {
        if self.close_saving {
            return;
        }
        if !force && self.view().is_some_and(|v| v.dirty) {
            self.confirm_close = true;
            self.palette_open = false;
            self.daily_group = 0;
            self.recent_open = false;
            self.drag_anchor = None;
            self.object_drag = None;
            self.bar_drag = None;
            self.reference_pick = None;
            window.focus(&self.close_focus, cx);
            cx.notify();
            return;
        }
        self.confirm_close = false;
        self.issue("window.close", json!({"force":force}), After::None, cx);
    }
    pub fn toolbar_button(
        &self,
        id: &'static str,
        label: impl Into<SharedString>,
        command: &'static str,
        params: Value,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let label = label.into();
        let keyboard_params = params.clone();
        div()
            .id(id)
            .role(Role::Button)
            .aria_label(label.clone())
            .tab_stop(true)
            .px_2()
            .h(px(29.))
            .flex()
            .items_center()
            .rounded_md()
            .cursor_pointer()
            .hover(|style| style.bg(rgb(if self.preferences.dark { 0x33433d } else { 0xe9f0ec })))
            .child(label)
            .on_key_down(cx.listener(move |this, event: &KeyDownEvent, window, cx| {
                if event.keystroke.key == "enter" || event.keystroke.key == "space" {
                    this.commit(After::Command(command.into(), keyboard_params.clone()), window, cx);
                    cx.stop_propagation();
                }
            }))
            .on_click(cx.listener(move |this, _, window, cx| this.commit(After::Command(command.into(), params.clone()), window, cx)))
    }
}

impl Render for Desktop {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let dark = self.preferences.dark;
        let bg = if dark { 0x172620 } else { 0xf4f6f5 };
        let panel = if dark { 0x20332a } else { 0xffffff };
        let fg = if dark { 0xe5eee8 } else { 0x26352d };
        let muted = if dark { 0x9eb0a4 } else { 0x6b8073 };
        let border = if dark { 0x354c3e } else { 0xe1e8e2 };
        let tabs: Vec<_> = self
            .view()
            .map(|v| {
                v.workbook
                    .sheets
                    .iter()
                    .enumerate()
                    .filter(|(_, s)| s.visibility == gridcraft_engine::model::Visibility::Visible)
                    .map(|(i, s)| (i, s.name.clone(), i == v.sheet))
                    .collect()
            })
            .unwrap_or_default();
        let address = self.selection().a1();
        let mode = if self.draft.is_some() { self.label("编辑中", "Editing") } else { self.label("就绪", "Ready") };
        let header = self.render_document_header(cx);
        let menu = self.render_ribbon_tabs(cx);
        let tools = self.render_ribbon_tools(cx);
        let formula = div()
            .h(px(36.))
            .flex_shrink_0()
            .flex()
            .items_center()
            .gap_2()
            .px_2()
            .py_1()
            .bg(rgb(bg))
            .border_b_1()
            .border_color(rgb(border))
            .child(Input::new(&self.name).id("name-box").w(px(98.)).h(px(30.)).aria_label("单元格地址 / Cell address"))
            .child(
                div()
                    .id("formula-cancel")
                    .role(Role::Button)
                    .aria_label(self.label("取消编辑", "Cancel edit"))
                    .cursor_pointer()
                    .px_2()
                    .text_color(rgb(muted))
                    .child("×")
                    .on_click(cx.listener(|this, _, w, cx| this.cancel_edit(w, cx))),
            )
            .child(
                div()
                    .id("formula-accept")
                    .role(Role::Button)
                    .aria_label(self.label("确认编辑", "Accept edit"))
                    .cursor_pointer()
                    .px_2()
                    .text_color(rgb(0x148256))
                    .child("✓")
                    .on_click(cx.listener(|this, _, w, cx| this.commit(After::None, w, cx))),
            )
            .child(div().w(px(26.)).text_color(rgb(muted)).text_center().child("fx"))
            .child(
                div()
                    .flex_1()
                    .key_context("JoysheetEditor")
                    .child(Input::new(&self.formula).id("formula-bar").w_full().h(px(30.)).aria_label("公式栏 / Formula bar")),
            );
        let grid = self.render_grid(window, cx);
        let sheet_tabs = div()
            .id("sheet-tab-list")
            .role(Role::TabList)
            .track_focus(&self.sheets_focus)
            .tab_stop(true)
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, w, cx| match event.keystroke.key.as_str() {
                "left" | "right" => {
                    this.commit(After::Command(if event.keystroke.key == "left" { "sheet.previous" } else { "sheet.next" }.into(), json!({})), w, cx);
                    cx.stop_propagation();
                }
                "enter" | "space" => {
                    this.focus_grid(w, cx);
                    cx.stop_propagation();
                }
                _ => {}
            }))
            .aria_label("工作表 / Worksheets")
            .h(px(32.))
            .flex_shrink_0()
            .flex()
            .items_center()
            .px_3()
            .gap_1()
            .bg(rgb(bg))
            .border_t_1()
            .border_color(rgb(border))
            .children(tabs.into_iter().map(|(index, name, active)| {
                div()
                    .id(("sheet", index))
                    .role(Role::Tab)
                    .tab_stop(true)
                    .aria_selected(active)
                    .aria_label(name.clone())
                    .px_4()
                    .h(px(30.))
                    .rounded_t_md()
                    .flex()
                    .items_center()
                    .cursor_pointer()
                    .bg(rgb(if active { panel } else { bg }))
                    .text_color(rgb(if active { 0x148256 } else { muted }))
                    .when(active, |el| el.font_weight(FontWeight::SEMIBOLD).border_b_2().border_color(rgb(0x148256)))
                    .relative()
                    .child(self.advanced_probe(format!("sheet-tab:{index}"), cx))
                    .child(name)
                    .on_key_down(cx.listener(move |this, event: &KeyDownEvent, w, cx| match event.keystroke.key.as_str() {
                        "enter" | "space" => {
                            this.commit(After::Command("sheet.activate".into(), json!({"sheet":index})), w, cx);
                            this.focus_grid(w, cx);
                            cx.stop_propagation();
                        }
                        "left" | "right" => {
                            this.commit(
                                After::Command(if event.keystroke.key == "left" { "sheet.previous" } else { "sheet.next" }.into(), json!({})),
                                w,
                                cx,
                            );
                            cx.stop_propagation();
                        }
                        _ => {}
                    }))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _, _, _| {
                            if let Some(v) = this.view() {
                                this.sheet_drag = Some((index, v.doc));
                            }
                        }),
                    )
                    .on_mouse_up(
                        MouseButton::Left,
                        cx.listener(move |this, _, w, cx| {
                            if let Some((from, doc)) = this.sheet_drag.take()
                                && from != index
                                && this.view().is_some_and(|v| v.doc == doc)
                            {
                                this.commit(After::Command("sheet.move".into(), json!({"sheet":from,"to":index})), w, cx);
                            }
                        }),
                    )
                    .on_click(cx.listener(move |this, _, window, cx| {
                        if this.draft.as_ref().is_some_and(|d| d.text.starts_with('=')) {
                            if let Some(d) = this.draft.as_mut() {
                                d.inline = false;
                            }
                            this.issue("sheet.activate", json!({"sheet":index}), After::None, cx);
                            this.formula.update(cx, |input, cx| input.focus(window, cx));
                        } else {
                            this.commit(After::Command("sheet.activate".into(), json!({"sheet":index})), window, cx);
                        }
                    }))
            }))
            .child(self.toolbar_button("add-sheet", "+", "home.insertSheet", json!({}), cx))
            .child(div().id("sheet-operations").px_3().cursor_pointer().child(self.label("工作表操作", "Sheets")).on_click(cx.listener(
                |this, _, _, cx| {
                    this.daily_group = 2;
                    cx.notify();
                },
            )))
            .child(div().flex_1());
        let status = div()
            .id("worksheet-status")
            .role(Role::Status)
            .a11y_synthetic_children(|builder| builder.parent_node().set_live(accesskit::Live::Polite))
            .h(px(25.))
            .flex_shrink_0()
            .flex()
            .items_center()
            .justify_between()
            .px_4()
            .text_xs()
            .bg(rgb(bg))
            .text_color(rgb(muted))
            .child(div().flex().gap_4().child(mode).child(address).child(
                div().max_w(px(650.)).overflow_hidden().text_color(rgb(if self.error { 0xc7473d } else { muted })).child(self.message.clone()),
            ))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(if cx.global::<crate::control::ControlRuntime>().diagnostics {
                        format!("{} cells · {:.2} ms paint · {:.2} ms engine", self.painted_cells, self.paint_ms, self.snapshot.command_ms)
                    } else {
                        self.label("本地工作簿", "Local workbook")
                    })
                    .child(div().id("zoom-out").px_2().cursor_pointer().child("−").on_click(cx.listener(|this, _, _, cx| this.zoom(-10, cx))))
                    .child(format!("{}%", self.view().and_then(|v| v.sheet()).map(|s| s.zoom).unwrap_or(100)))
                    .child(div().id("zoom-in").px_2().cursor_pointer().child("+").on_click(cx.listener(|this, _, _, cx| this.zoom(10, cx)))),
            );
        let mut content = div()
            .id("joysheet")
            .on_drop(cx.listener(|this, paths: &ExternalPaths, window, cx| match joysheet_platform::workbook_paths(paths.paths().iter().cloned()) {
                Ok(paths) => this.commit(After::OpenPaths(paths), window, cx),
                Err(e) => {
                    this.alert(e);
                    cx.notify();
                }
            }))
            .size_full()
            .flex()
            .flex_col()
            .relative()
            .bg(rgb(bg))
            .text_color(rgb(fg))
            .text_sm()
            .on_action(cx.listener(|this, _: &crate::NewWorkbook, window, cx| this.commit(After::Command("file.new".into(), json!({})), window, cx)))
            .on_action(cx.listener(|this, _: &crate::OpenWorkbook, window, cx| this.pick_open(window, cx)))
            .on_action(cx.listener(|this, _: &crate::SaveWorkbook, window, cx| this.commit(After::Save(false), window, cx)))
            .on_action(cx.listener(|this, _: &crate::SaveAsWorkbook, window, cx| this.commit(After::Save(true), window, cx)))
            .on_action(cx.listener(|this, _: &crate::CloseWorkbook, window, cx| this.commit(After::Close(false), window, cx)))
            .on_action(
                cx.listener(|this, _: &crate::UndoWorkbook, window, cx| this.commit(After::Command("edit.undo".into(), json!({})), window, cx)),
            )
            .on_action(
                cx.listener(|this, _: &crate::RedoWorkbook, window, cx| this.commit(After::Command("edit.redo".into(), json!({})), window, cx)),
            )
            .on_action(cx.listener(|this, _: &crate::ShowCommands, window, cx| {
                this.palette_open = true;
                this.search.update(cx, |input, cx| input.focus(window, cx));
                cx.notify();
            }))
            .capture_key_down(cx.listener(|this, event, window, cx| this.key_down(event, window, cx)))
            .capture_action(cx.listener(|this, _: &gpui_kit::component::input::Escape, w, cx| {
                this.key_down(
                    &KeyDownEvent {
                        keystroke: Keystroke { key: "escape".into(), key_char: None, modifiers: Modifiers::default() },
                        is_held: false,
                        prefer_character_input: false,
                    },
                    w,
                    cx,
                )
            }))
            .capture_action(cx.listener(|this, _: &gpui_base::actions::Cancel, w, cx| {
                this.key_down(
                    &KeyDownEvent {
                        keystroke: Keystroke { key: "escape".into(), key_char: None, modifiers: Modifiers::default() },
                        is_held: false,
                        prefer_character_input: false,
                    },
                    w,
                    cx,
                )
            }))
            .capture_action(
                cx.listener(|this, _: &gpui_kit::component::input::MoveRight, w, cx| this.capture_input_navigation("right", false, false, w, cx)),
            )
            .capture_action(
                cx.listener(|this, _: &gpui_kit::component::input::MoveLeft, w, cx| this.capture_input_navigation("left", false, false, w, cx)),
            )
            .capture_action(
                cx.listener(|this, _: &gpui_kit::component::input::MoveUp, w, cx| this.capture_input_navigation("up", false, false, w, cx)),
            )
            .capture_action(
                cx.listener(|this, _: &gpui_kit::component::input::MoveDown, w, cx| this.capture_input_navigation("down", false, false, w, cx)),
            )
            .capture_action(
                cx.listener(|this, _: &gpui_base::actions::SelectRight, w, cx| this.capture_input_navigation("right", true, false, w, cx)),
            )
            .capture_action(cx.listener(|this, _: &gpui_base::actions::SelectLeft, w, cx| this.capture_input_navigation("left", true, false, w, cx)))
            .capture_action(cx.listener(|this, _: &gpui_base::actions::SelectUp, w, cx| this.capture_input_navigation("up", true, false, w, cx)))
            .capture_action(cx.listener(|this, _: &gpui_base::actions::SelectDown, w, cx| this.capture_input_navigation("down", true, false, w, cx)))
            .capture_action(
                cx.listener(|this, _: &gpui_kit::component::input::SelectAll, w, cx| this.capture_input_navigation("a", false, true, w, cx)),
            )
            .capture_action(
                cx.listener(|this, _: &gpui_kit::component::input::MoveHome, w, cx| this.capture_input_navigation("home", false, false, w, cx)),
            )
            .capture_action(
                cx.listener(|this, _: &gpui_kit::component::input::MoveEnd, w, cx| this.capture_input_navigation("end", false, false, w, cx)),
            )
            .capture_action(
                cx.listener(|this, _: &gpui_kit::component::input::MovePageUp, w, cx| this.capture_input_navigation("pageup", false, false, w, cx)),
            )
            .capture_action(
                cx.listener(|this, _: &gpui_kit::component::input::MovePageDown, w, cx| {
                    this.capture_input_navigation("pagedown", false, false, w, cx)
                }),
            )
            .capture_action(
                cx.listener(|this, _: &gpui_kit::component::input::MoveToStart, w, cx| this.capture_input_navigation("home", false, true, w, cx)),
            )
            .capture_action(
                cx.listener(|this, _: &gpui_kit::component::input::MoveToEnd, w, cx| this.capture_input_navigation("end", false, true, w, cx)),
            )
            .capture_action(cx.listener(|this, _: &gpui_kit::component::input::MoveToPreviousWord, w, cx| {
                this.capture_input_navigation("left", false, true, w, cx)
            }))
            .capture_action(
                cx.listener(|this, _: &gpui_kit::component::input::MoveToNextWord, w, cx| this.capture_input_navigation("right", false, true, w, cx)),
            )
            .capture_action(cx.listener(|this, _: &gpui_kit::component::input::SelectToStartOfLine, w, cx| {
                this.capture_input_navigation("home", true, false, w, cx)
            }))
            .capture_action(
                cx.listener(|this, _: &gpui_kit::component::input::SelectToEndOfLine, w, cx| {
                    this.capture_input_navigation("end", true, false, w, cx)
                }),
            )
            .capture_action(
                cx.listener(|this, _: &gpui_kit::component::input::SelectToStart, w, cx| this.capture_input_navigation("home", true, true, w, cx)),
            )
            .capture_action(
                cx.listener(|this, _: &gpui_kit::component::input::SelectToEnd, w, cx| this.capture_input_navigation("end", true, true, w, cx)),
            )
            .capture_action(cx.listener(|this, _: &gpui_kit::component::input::SelectToPreviousWordStart, w, cx| {
                this.capture_input_navigation("left", true, true, w, cx)
            }))
            .capture_action(cx.listener(|this, _: &gpui_kit::component::input::SelectToNextWordEnd, w, cx| {
                this.capture_input_navigation("right", true, true, w, cx)
            }))
            .capture_action(
                cx.listener(|this, _: &gpui_kit::component::input::ShowCompletions, w, cx| {
                    this.capture_input_navigation("space", false, true, w, cx)
                }),
            )
            .capture_action(
                cx.listener(|this, _: &gpui_kit::component::input::Delete, w, cx| this.capture_input_navigation("delete", false, false, w, cx)),
            )
            .capture_action(
                cx.listener(|this, _: &gpui_kit::component::input::Backspace, w, cx| this.capture_input_navigation("backspace", false, false, w, cx)),
            )
            .capture_action(cx.listener(|this, action: &crate::WorksheetKey, window, cx| {
                let modifiers = Modifiers {
                    shift: action.shift,
                    control: action.command && !cfg!(target_os = "macos"),
                    platform: action.command && cfg!(target_os = "macos"),
                    ..Default::default()
                };
                this.key_down(
                    &KeyDownEvent {
                        keystroke: Keystroke { modifiers, key: action.key.clone(), key_char: None },
                        is_held: false,
                        prefer_character_input: false,
                    },
                    window,
                    cx,
                );
            }))
            .capture_action(cx.listener(|this, _: &gpui_kit::component::input::Undo, window, cx| {
                this.capture_text_command("edit.undo", window, cx);
            }))
            .capture_action(cx.listener(|this, _: &gpui_kit::component::input::Redo, window, cx| {
                this.capture_text_command("edit.redo", window, cx);
            }))
            .capture_action(cx.listener(|this, _: &gpui_kit::component::input::Copy, window, cx| {
                this.capture_text_command("edit.copy", window, cx);
            }))
            .capture_action(cx.listener(|this, _: &gpui_kit::component::input::Cut, window, cx| {
                this.capture_text_command("edit.cut", window, cx);
            }))
            .capture_action(cx.listener(|this, _: &gpui_kit::component::input::Paste, window, cx| {
                this.capture_text_command("edit.paste", window, cx);
            }))
            .capture_any_mouse_down(cx.listener(|this, _, _, cx| {
                if this.snapshot.active_window != this.window_id
                    && let Some(id) = this.window_id
                {
                    this.issue("window.activate", json!({"window":id}), After::None, cx);
                }
            }))
            .child(header)
            .child(menu)
            .child(tools)
            .when(self.preferences.formula_bar, |el| el.child(formula))
            .child(grid)
            .child(sheet_tabs)
            .child(status);
        if self.palette_open {
            content = content.child(self.render_palette(window, cx));
        }
        if self.daily_group == 22 {
            content = content.child(self.object_panel(cx));
        } else if self.daily_group == 21 {
            content = content.child(self.recovery_panel(cx));
        } else if self.daily_group == 20 {
            content = content.child(self.settings_panel(cx));
        } else if self.daily_group == 18 {
            content = content.child(self.pivot_fields_panel(cx));
        } else if self.daily_group == 19 {
            content = content.child(self.names_panel(cx));
        } else if self.daily_group == 16 {
            content = content.child(self.script_panel(cx));
        } else if self.daily_group == 17 {
            content = content.child(self.control_panel(cx));
        } else if self.daily_group != 0 {
            content = content.child(self.daily_panel(cx));
        }
        if self.preview.is_some() {
            content = content.child(self.render_preview(cx));
        }
        if !self.palette_open
            && let Some(hint) = self.formula_hint(cx)
        {
            content = content.child(
                div()
                    .absolute()
                    .top(px(172.))
                    .left(px(150.))
                    .max_w(px(610.))
                    .p_3()
                    .rounded_md()
                    .bg(rgb(panel))
                    .border_1()
                    .border_color(rgb(border))
                    .shadow_md()
                    .text_sm()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(StyledText::new(hint.signature).with_highlights([(
                        hint.active,
                        HighlightStyle { font_weight: Some(FontWeight::BOLD), color: Some(rgb(0x148256).into()), ..Default::default() },
                    )]))
                    .child(hint.description)
                    .child(
                        div().flex().flex_wrap().gap_2().children(
                            self.draft
                                .as_ref()
                                .map(|d| crate::formula_feedback::references(&d.text))
                                .unwrap_or_default()
                                .into_iter()
                                .map(|r| div().text_color(rgb(crate::formula_feedback::COLORS[r.color])).child(r.label)),
                        ),
                    ),
            );
        }
        if self.validation_prompt.is_some() {
            content = content.child(self.validation_dialog(cx));
        }
        if self.confirm_close {
            content = content.child(self.render_close_dialog(cx));
        }
        if self.trust_request.is_some() {
            content = content.child(self.trust_dialog(cx));
        }
        if self.recent_open {
            let mut list = div()
                .occlude()
                .absolute()
                .left(px(300.))
                .top(px(88.))
                .w(px(450.))
                .p_4()
                .rounded_lg()
                .shadow_xl()
                .bg(rgb(panel))
                .flex()
                .flex_col()
                .gap_2()
                .child(div().font_weight(FontWeight::SEMIBOLD).child(self.label("最近文件", "Recent files")))
                .child(div().id("close-recents").cursor_pointer().child(self.label("关闭列表", "Close list")).on_click(cx.listener(
                    |this, _, _, cx| {
                        this.recent_open = false;
                        cx.notify();
                    },
                )));
            for (index, path) in self.preferences.recent.iter().take(10).cloned().enumerate() {
                list = list.child(div().id(("recent", index)).cursor_pointer().p_2().text_sm().child(path.clone()).on_click(cx.listener(
                    move |this, _, window, cx| {
                        this.recent_open = false;
                        this.commit(After::Command("file.open".into(), json!({"path":path})), window, cx);
                    },
                )));
            }
            if self.preferences.recent.is_empty() {
                list = list.child(self.label("还没有最近打开的文件", "No recent files"));
            }
            content = content.child(list);
        }
        content
    }
}
