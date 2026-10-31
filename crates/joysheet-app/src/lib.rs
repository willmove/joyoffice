//! A single engine worker owns Session. Views receive immutable workbook snapshots.
#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]
pub mod control;
pub mod geometry;
mod notifications;
pub mod profiling;
pub mod recovery;
pub mod smoke;
pub mod tasks;

use joysheet_engine::{
    Selection, Session, Target, UiRequest,
    model::{Sheet, Workbook},
};
use joysheet_protocol::{CommandTarget, MAX_PENDING_COMMANDS};
use serde_json::{Value, json};
use std::sync::{
    Arc, RwLock,
    atomic::{AtomicU64, Ordering},
    mpsc,
};
use std::time::{Duration, Instant};

#[derive(Clone, Debug)]
pub struct WindowSnapshot {
    pub window: u64,
    pub doc: u64,
    pub revision: u64,
    pub title: String,
    pub path: Option<String>,
    pub dirty: bool,
    pub sheet: usize,
    pub selection: Selection,
    pub workbook: Arc<Workbook>,
    pub undo: Vec<String>,
    pub redo: Vec<String>,
    pub scripts_trusted: bool,
    pub recording: bool,
    pub show_all_notes: bool,
}
impl WindowSnapshot {
    pub fn sheet(&self) -> Option<&Sheet> {
        self.workbook.sheets.get(self.sheet).map(Arc::as_ref)
    }
}
#[derive(Clone, Debug, Default)]
pub struct AppSnapshot {
    pub sequence: u64,
    pub active_window: Option<u64>,
    pub windows: Vec<WindowSnapshot>,
    pub command_ms: f64,
}
impl AppSnapshot {
    pub fn active(&self) -> Option<&WindowSnapshot> {
        self.windows.iter().find(|w| Some(w.window) == self.active_window)
    }
    pub fn window(&self, id: u64) -> Option<&WindowSnapshot> {
        self.windows.iter().find(|w| w.window == id)
    }
    pub fn inspect(&self) -> Value {
        json!({"sequence":self.sequence,"activeWindow":self.active_window,"commandMs":self.command_ms,
            "windows":self.windows.iter().map(|w|json!({"window":w.window,"doc":w.doc,"title":w.title,
                "dirty":w.dirty,"path":w.path,"revision":w.revision,"sheet":w.sheet,
                "selection":w.selection.a1(),"active":Some(w.window)==self.active_window})).collect::<Vec<_>>()})
    }
}
#[derive(Clone, Debug)]
pub struct Completion {
    pub request: u64,
    pub result: Result<Value, String>,
    pub requests: Vec<UiRequest>,
    pub snapshot: Arc<AppSnapshot>,
    pub timing: profiling::CommandTiming,
    pub ready_at: Instant,
}
pub struct Ticket {
    pub id: u64,
    receiver: mpsc::Receiver<Completion>,
}
impl Ticket {
    pub fn poll(&self) -> Option<Result<Completion, String>> {
        match self.receiver.try_recv() {
            Ok(value) => Some(Ok(value)),
            Err(mpsc::TryRecvError::Empty) => None,
            Err(mpsc::TryRecvError::Disconnected) => Some(Err("engine worker stopped".into())),
        }
    }
    pub fn wait(self, timeout: Duration) -> Result<Completion, String> {
        self.receiver.recv_timeout(timeout).map_err(|e| format!("engine response: {e}"))
    }
}
struct Request {
    enqueued_at: Instant,
    control: Option<joysheet_engine::ScriptControl>,
    id: u64,
    command: String,
    params: Value,
    target: CommandTarget,
    expected_revision: Option<u64>,
    reply: mpsc::Sender<Completion>,
    text_width: Option<joysheet_engine::TextWidth>,
}
#[derive(Clone)]
pub struct EngineClient {
    notifications: notifications::Notifications,
    profile: profiling::CommandHistory,
    tasks: tasks::Tasks,
    sender: mpsc::SyncSender<Request>,
    latest: Arc<RwLock<Arc<AppSnapshot>>>,
    next_id: Arc<AtomicU64>,
}
impl EngineClient {
    pub fn start() -> Result<Self, String> {
        let (sender, receiver) = mpsc::sync_channel::<Request>(MAX_PENDING_COMMANDS);
        let latest = Arc::new(RwLock::new(Arc::new(AppSnapshot::default())));
        let published = latest.clone();
        let tasks = tasks::Tasks::default();
        let worker_tasks = tasks.clone();
        let profile = profiling::CommandHistory::default();
        let worker_profile = profile.clone();
        let notifications = notifications::Notifications::default();
        let worker_notifications = notifications.clone();
        std::thread::Builder::new()
            .name("joysheet-engine".into())
            .spawn(move || {
                let mut session = Session::new();
                let mut sequence = 0u64;
                while let Ok(request) = receiver.recv() {
                    let started = Instant::now();
                    let queue_ms = started.duration_since(request.enqueued_at).as_secs_f64() * 1000.;
                    if request.control.is_some() {
                        let window = match (request.target.window, request.target.doc) {
                            (Some(id), _) => session.windows().iter().find(|w| w.id == id),
                            (_, Some(doc)) => session.windows().iter().find(|w| w.doc_uid == doc),
                            _ => session.active_window().and_then(|id| session.windows().iter().find(|w| w.id == id)),
                        };
                        if let Some(task) = worker_tasks.lock().unwrap_or_else(std::sync::PoisonError::into_inner).get_mut(&request.id) {
                            task.info.window = window.map(|w| w.id);
                            task.info.doc = window.map(|w| w.doc_uid);
                        }
                    }
                    tasks::state(&worker_tasks, request.id, "running", String::new());
                    session.script_control = request.control.clone();
                    let execute_started = Instant::now();
                    joysheet_engine::calc::profile::start();
                    let result = if request.control.as_ref().is_some_and(|c| c.cancelled.load(Ordering::Acquire)) {
                        Err("script cancelled before execution".into())
                    } else {
                        execute_request(&mut session, &request)
                    };
                    let execute_ms = execute_started.elapsed().as_secs_f64() * 1000.;
                    let calculation = profiling::calculation(joysheet_engine::calc::profile::finish());
                    session.script_control = None;
                    let state = if result.is_ok() {
                        "completed"
                    } else if request.control.as_ref().is_some_and(|c| c.cancelled.load(Ordering::Acquire)) {
                        "cancelled"
                    } else {
                        "failed"
                    };
                    tasks::state(&worker_tasks, request.id, state, result.as_ref().err().cloned().unwrap_or_default());
                    let requests = session.take_ui_requests();
                    sequence = sequence.saturating_add(1);
                    let snapshot_started = Instant::now();
                    let snapshot = Arc::new(capture(&mut session, sequence, started.elapsed().as_secs_f64() * 1000.0));
                    let snapshot_ms = snapshot_started.elapsed().as_secs_f64() * 1000.;
                    let publish_started = Instant::now();
                    *published.write().unwrap_or_else(std::sync::PoisonError::into_inner) = snapshot.clone();
                    let timing = profiling::CommandTiming {
                        calculation,
                        queue_ms,
                        execute_ms,
                        snapshot_ms,
                        publish_ms: publish_started.elapsed().as_secs_f64() * 1000.,
                    };
                    worker_profile.record(request.id, &request.command, sequence, timing.clone());
                    let _ = request.reply.send(Completion { request: request.id, result, requests, snapshot, timing, ready_at: Instant::now() });
                    // Reply and latest are visible before any UI subscriber wakes.
                    worker_notifications.publish();
                }
            })
            .map_err(|e| format!("start engine: {e}"))?;
        Ok(Self { sender, latest, next_id: Arc::new(AtomicU64::new(1)), tasks, profile, notifications })
    }
    pub fn subscribe(&self) -> async_channel::Receiver<()> {
        self.notifications.subscribe()
    }
    pub fn latest(&self) -> Arc<AppSnapshot> {
        self.latest.read().unwrap_or_else(std::sync::PoisonError::into_inner).clone()
    }
    pub fn submit(&self, command: &str, params: Value, target: CommandTarget) -> Result<Ticket, String> {
        self.submit_guarded(command, params, target, None)
    }
    pub fn submit_guarded(&self, command: &str, params: Value, target: CommandTarget, expected_revision: Option<u64>) -> Result<Ticket, String> {
        if !params.is_object() {
            return Err("command parameters must be an object".into());
        }
        if serde_json::to_vec(&params).map_err(|e| e.to_string())?.len() > joysheet_protocol::MAX_LINE_BYTES {
            return Err("command exceeds 8 MiB".into());
        }
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let (reply, receiver) = mpsc::channel();
        let control = (command == "automate.runScript").then(joysheet_engine::ScriptControl::default);
        if let Some(control) = &control {
            let snapshot = self.latest();
            let window = match (target.window, target.doc) {
                (Some(id), _) => Some(id),
                (_, Some(doc)) => snapshot.windows.iter().find(|w| w.doc == doc).map(|w| w.window),
                _ => snapshot.active_window,
            };
            let doc = target.doc.or_else(|| window.and_then(|id| snapshot.window(id).map(|w| w.doc)));
            let mut jobs = self.tasks.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
            while jobs.len() >= 64 {
                let old = jobs.iter().find(|(_, t)| !matches!(t.info.state.as_str(), "queued" | "running" | "cancelling")).map(|(id, _)| *id);
                if let Some(id) = old {
                    jobs.remove(&id);
                } else {
                    break;
                }
            }
            jobs.insert(
                id,
                tasks::Task {
                    info: tasks::TaskInfo {
                        id,
                        command: command.into(),
                        window,
                        doc,
                        state: "queued".into(),
                        completed: 0,
                        cancellable: true,
                        message: String::new(),
                    },
                    control: control.clone(),
                },
            );
        }
        self.sender
            .try_send(Request {
                enqueued_at: Instant::now(),
                id,
                command: command.into(),
                params,
                target,
                expected_revision,
                reply,
                text_width: None,
                control,
            })
            .map_err(|e| {
                tasks::state(&self.tasks, id, "failed", e.to_string());
                format!("engine queue unavailable: {e}")
            })?;
        Ok(Ticket { id, receiver })
    }
    pub fn install_text_width(&self, text_width: joysheet_engine::TextWidth) -> Result<(), String> {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let (reply, _) = mpsc::channel();
        self.sender
            .try_send(Request {
                enqueued_at: Instant::now(),
                id,
                command: "__joysheet.text_width".into(),
                params: json!({}),
                target: CommandTarget::default(),
                expected_revision: None,
                reply,
                text_width: Some(text_width),
                control: None,
            })
            .map_err(|e| e.to_string())
    }
    /// Blocking helper for CLI, control and tests. GPUI uses submit + poll.
    pub fn execute(&self, command: &str, params: Value) -> Result<Completion, String> {
        self.submit(command, params, CommandTarget::default())?.wait(Duration::from_secs(60))
    }
    pub fn dispatch(&self, method: &str, params: Value) -> Result<Value, String> {
        if method == "engine.profile" {
            return Ok(self.profile.inspect());
        }
        if matches!(method, "engine.tasks" | "engine.cancel") {
            let target = CommandTarget::from_params(&params)?;
            let snapshot = self.latest();
            if target.window.is_some_and(|id| snapshot.window(id).is_none())
                || target.doc.is_some_and(|id| !snapshot.windows.iter().any(|w| w.doc == id))
            {
                return Err("target document/window is unavailable".into());
            }
            let tasks: Vec<_> = self
                .tasks()
                .into_iter()
                .filter(|t| target.window.is_none_or(|id| t.window == Some(id)) && target.doc.is_none_or(|id| t.doc == Some(id)))
                .collect();
            if method == "engine.tasks" {
                return Ok(json!(tasks));
            }
            let id = params.get("id").and_then(Value::as_u64).ok_or("missing task id")?;
            if !tasks.iter().any(|t| t.id == id) {
                return Err("task does not belong to the target".into());
            }
            return Ok(json!({"cancelled":self.cancel_task(id)?}));
        }
        if method == "engine.journal" {
            return self.execute("__joysheet.journal", params)?.result;
        }
        if method == "engine.commands" {
            return self.execute("app.commands", params)?.result;
        }
        if method == "ui.inspect" {
            return Ok(self.latest().inspect());
        }
        let target = CommandTarget::from_params(&params)?;
        let (command, params) = match method {
            "engine.execute" => (
                params.get("command").and_then(Value::as_str).ok_or("missing command")?.to_string(),
                params.get("params").cloned().unwrap_or_else(|| json!({})),
            ),
            "app.open" => ("file.open".into(), params),
            "app.save" => ("file.save".into(), params),
            _ => (method.into(), params),
        };
        let completion = self.submit(&command, params, target)?.wait(Duration::from_secs(60))?;
        if completion.requests.iter().any(|r| matches!(r, UiRequest::Dialog(_, _))) {
            return Err("command requires explicit parameters; programmatic calls do not open dialogs".into());
        }
        completion.result
    }
    pub fn tasks(&self) -> Vec<tasks::TaskInfo> {
        tasks::snapshot(&self.tasks)
    }
    pub fn cancel_task(&self, id: u64) -> Result<bool, String> {
        let mut tasks = self.tasks.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        let t = tasks.get_mut(&id).ok_or("No such script task")?;
        if !matches!(t.info.state.as_str(), "queued" | "running" | "cancelling") {
            return Ok(false);
        }
        t.control.cancelled.store(true, Ordering::Release);
        t.info.state = "cancelling".into();
        Ok(true)
    }
}
fn execute_request(s: &mut Session, r: &Request) -> Result<Value, String> {
    let target = match (r.target.window, r.target.doc) {
        (Some(w), None) => Target::Window(w),
        (None, Some(d)) => Target::Doc(d),
        (None, None) => Target::Active,
        _ => return Err("specify either window or doc".into()),
    };
    let run = |s: &mut Session| {
        if r.command == "__joysheet.nativeActivate" {
            if r.params.get("previousActive").and_then(Value::as_u64) != s.active_window() {
                return Ok(json!({"ignored":true}));
            }
            return s.execute("window.activate", json!({"window":r.params["window"]})).map_err(|e| e.to_string());
        }
        if r.command == "__joysheet.text_width" {
            s.text_width = r.text_width.clone();
            return Ok(Value::Null);
        }
        if r.command == "__joysheet.journal" {
            return Ok(json!(s.journal));
        }
        if r.command == "__joysheet.scriptDetails" {
            let d = s.active().ok_or("No document")?;
            let scripts = joysheet_engine::cmd::extra::script_definitions(&d.wb);
            return Ok(scripts);
        }
        if r.command == "__joysheet.previewImport" {
            let mut preview = Session::new();
            preview.execute("file.new", json!({})).map_err(|e| e.to_string())?;
            preview.execute("data.getData", r.params.clone()).map_err(|e| e.to_string())?;
            let doc = preview.active().ok_or("No preview document")?;
            let sheet = doc.wb.sheet(doc.sheet_index()).ok_or("No preview sheet")?;
            let used = sheet.used_range().ok_or("No data")?;
            let rows: Vec<_> = (used.start.row..=used.end.row)
                .take(20)
                .map(|row| {
                    let values: Vec<_> = (used.start.col..=used.end.col)
                        .take(10)
                        .map(|col| sheet.value(joysheet_engine::core::CellRef::new(row, col)).display().chars().take(256).collect::<String>())
                        .collect();
                    json!({"name":format!("行 {}",row+1),"value":values.join(" | ")})
                })
                .collect();
            return Ok(json!({"results":rows,"rows":used.height(),"columns":used.width(),"previewOnly":true}));
        }
        if let Some(expected) = r.expected_revision
            && s.active().is_none_or(|d| d.revision != expected)
        {
            return Err("document changed; stale operation was rejected".into());
        }
        if r.command == "__joysheet.objectBatch" {
            if r.params.get("sheet").and_then(Value::as_u64) != s.active().map(|d| d.sheet_index() as u64) {
                return Err("Object worksheet changed; select the objects again".into());
            }
            let operations =
                r.params["operations"].as_array().filter(|a| !a.is_empty() && a.len() <= 256).ok_or("Select between 1 and 256 object operations")?;
            for operation in operations {
                if !operation["command"]
                    .as_str()
                    .is_some_and(|c| ["object.move", "object.delete", "arrange.bringForward", "arrange.sendBackward"].contains(&c))
                    || !operation["params"].is_object()
                {
                    return Err("Only object move/delete/stacking commands are allowed in this batch".into());
                }
            }
            return s
                .grouped("Objects", |session| {
                    for operation in operations {
                        if let Some(command) = operation["command"].as_str() {
                            session.execute(command, operation["params"].clone())?;
                        }
                    }
                    Ok(json!({"objects":operations.len()}))
                })
                .map_err(|e| e.to_string());
        }
        if r.command == "__joysheet.validatedSet" {
            let issue = s.active().and_then(|d| {
                let at = r.params.get("cell").and_then(Value::as_str).and_then(joysheet_engine::core::CellRef::parse)?;
                let text = r.params.get("input").and_then(Value::as_str)?;
                joysheet_engine::cmd::data::check_validation(
                    &d.wb,
                    r.params.get("sheet").and_then(Value::as_u64).map(|i| i as usize).unwrap_or(d.sheet_index()),
                    at,
                    text,
                )
                .filter(|(rule, _)| rule.show_error)
                .map(|(rule, message)| (rule.error_style == joysheet_engine::model::ErrorStyle::Stop, message))
            });
            if let Some((stop, message)) = issue {
                if stop {
                    return Err(message);
                }
                if r.params.get("validationAccepted").and_then(Value::as_bool) != Some(true) {
                    return Err(format!("Validation confirmation required: {message}"));
                }
            }
            return s.execute("cell.set", r.params.clone()).map_err(|e| e.to_string());
        }
        s.execute(&r.command, r.params.clone()).map_err(|e| e.to_string())
    };
    if target == Target::Active { run(s) } else { s.with_target(target, run).map_err(|e| e.to_string())? }
}
fn capture(s: &mut Session, sequence: u64, command_ms: f64) -> AppSnapshot {
    let active_window = s.active_window();
    let ids: Vec<u64> = s.windows().iter().filter(|w| !w.hidden).map(|w| w.id).collect();
    let mut windows = Vec::new();
    for id in ids {
        if let Ok(Some(view)) = s.with_target(Target::Window(id), |s| {
            let d = s.active()?;
            Some(WindowSnapshot {
                window: id,
                doc: d.uid,
                revision: d.revision,
                title: s.window_title(id).unwrap_or_else(|| d.title.clone()),
                path: d.path.clone(),
                dirty: d.is_dirty(),
                sheet: d.sheet_index(),
                selection: d.selection.clone(),
                workbook: d.wb.clone(),
                undo: d.undo.iter().map(|h| h.label.clone()).collect(),
                redo: d.redo.iter().map(|h| h.label.clone()).collect(),
                scripts_trusted: d.scripts_trusted,
                recording: s.recording.is_some(),
                show_all_notes: s.show_all_notes,
            })
        }) {
            windows.push(view);
        }
    }
    AppSnapshot { sequence, active_window, windows, command_ms }
}
#[cfg(test)]
#[path = "tests/lib_tests.rs"]
mod tests;
