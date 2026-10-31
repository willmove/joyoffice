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
mod tests {
    #[test]
    fn calculation_phases_belong_to_the_current_command_only() {
        let e = EngineClient::start().unwrap();
        run(&e, "file.new", json!({}));
        run(&e, "cell.set", json!({"cell":"A1","input":"2"}));
        let c = run(&e, "cell.set", json!({"cell":"B1","input":"=SUM(A1:A5)"}));
        assert_eq!(c.timing.calculation["formulasEvaluated"], 1);
        assert!(c.timing.calculation["rangesRead"].as_u64().unwrap() >= 1);
        for key in ["recalcMs", "dependencyMs", "evaluationMs", "writebackMs", "rangeReadMs", "usedRangeMs", "rowFitMs"] {
            assert!(c.timing.calculation[key].as_f64().unwrap() >= 0.);
        }
        assert_eq!(run(&e, "cell.get", json!({"cell":"B1"})).timing.calculation["formulasEvaluated"], 0);
        assert_eq!(run(&e, "cell.get", json!({"cell":"B1"})).result.unwrap()["value"].as_f64(), Some(2.));
    }
    #[test]
    fn completion_wakeup_observes_both_reply_and_snapshot() {
        let engine = super::EngineClient::start().unwrap();
        let updates = engine.subscribe();
        updates.try_recv().unwrap();
        let ticket = engine.submit("file.new", serde_json::json!({}), Default::default()).unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while updates.try_recv().is_err() {
            assert!(std::time::Instant::now() < deadline);
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        let c = ticket.poll().unwrap().unwrap();
        assert!(c.result.is_ok());
        assert_eq!(engine.latest().sequence, c.snapshot.sequence);
        assert!(engine.latest().active().is_some());
    }
    use super::*;
    fn run(e: &EngineClient, c: &str, p: Value) -> Completion {
        let r = e.execute(c, p).unwrap();
        assert!(r.result.is_ok(), "{:?}", r.result);
        r
    }
    #[test]
    fn queued_edits_snapshots_and_undo() {
        let e = EngineClient::start().unwrap();
        run(&e, "file.new", json!({}));
        let before = e.latest();
        let a = e.submit("cell.set", json!({"cell":"A1","input":"10"}), CommandTarget::default()).unwrap();
        let b = e.submit("cell.set", json!({"cell":"B1","input":"=A1*2"}), CommandTarget::default()).unwrap();
        a.wait(Duration::from_secs(5)).unwrap().result.unwrap();
        b.wait(Duration::from_secs(5)).unwrap().result.unwrap();
        assert!(before.active().unwrap().sheet().unwrap().cells.is_empty());
        assert_eq!(run(&e, "cell.get", json!({"cell":"B1"})).result.unwrap()["value"], 20.0);
        run(&e, "edit.undo", json!({}));
        assert!(run(&e, "cell.get", json!({"cell":"B1"})).result.unwrap()["value"].is_null());
    }
    #[test]
    fn guards_and_targets() {
        let e = EngineClient::start().unwrap();
        run(&e, "file.new", json!({}));
        let revision = e.latest().active().unwrap().revision;
        let first = e.latest().active_window.unwrap();
        run(&e, "cell.set", json!({"input":"1"}));
        assert!(
            e.submit_guarded("cell.set", json!({"input":"2"}), CommandTarget::default(), Some(revision))
                .unwrap()
                .wait(Duration::from_secs(5))
                .unwrap()
                .result
                .is_err()
        );
        run(&e, "file.new", json!({}));
        let front = e.latest().active_window;
        e.dispatch("engine.execute", json!({"window":first,"command":"cell.set","params":{"cell":"A1","input":"中文"}})).unwrap();
        assert_eq!(e.latest().active_window, front);
        assert_eq!(e.dispatch("cell.get", json!({"window":first,"cell":"A1"})).unwrap()["value"], "中文");
        assert!(e.dispatch("cell.get", json!({"window":99999})).is_err());
    }
    #[test]
    fn script_task_identity_follows_target_document() {
        let e = EngineClient::start().unwrap();
        run(&e, "file.new", json!({}));
        let (doc, first) = (e.latest().active().unwrap().doc, e.latest().active_window.unwrap());
        run(&e, "automate.newScript", json!({"name":"empty","commands":[]}));
        run(&e, "file.new", json!({}));
        let second = e.latest().active_window.unwrap();
        let ticket = e.submit("automate.runScript", json!({"name":"empty"}), CommandTarget { window: None, doc: Some(doc) }).unwrap();
        let id = ticket.id;
        ticket.wait(Duration::from_secs(5)).unwrap().result.unwrap();
        let tasks = e.dispatch("engine.tasks", json!({"doc":doc})).unwrap();
        assert_eq!(tasks[0]["id"], id);
        assert_eq!(tasks[0]["window"], first);
        assert_eq!(tasks[0]["state"], "completed");
        assert!(e.dispatch("engine.tasks", json!({"window":second})).unwrap().as_array().unwrap().is_empty());
        assert!(e.dispatch("engine.cancel", json!({"window":second,"id":id})).is_err());
        assert_eq!(e.dispatch("engine.cancel", json!({"window":first,"id":id})).unwrap()["cancelled"], false);
        assert_eq!(e.latest().active_window, Some(second));
    }
    #[test]
    fn validated_edit_rejects_without_mutation_and_keeps_undo_semantics() {
        let e = EngineClient::start().unwrap();
        run(&e, "file.new", json!({}));
        run(&e, "data.validation", json!({"range":"A1","type":"whole","formula1":"0","formula2":"100"}));
        assert!(e.execute("__joysheet.validatedSet", json!({"cell":"A1","input":"999"})).unwrap().result.is_err());
        assert!(run(&e, "cell.get", json!({"cell":"A1"})).result.unwrap()["value"].is_null());
        run(&e, "__joysheet.validatedSet", json!({"cell":"A1","input":"50"}));
        run(&e, "edit.undo", json!({}));
        assert!(run(&e, "cell.get", json!({"cell":"A1"})).result.unwrap()["value"].is_null());
    }
    #[test]
    fn warning_confirmation_targets_explicit_sheet_and_keeps_undo() {
        let e = EngineClient::start().unwrap();
        run(&e, "file.new", json!({}));
        run(&e, "data.validation", json!({"range":"A1","type":"whole","formula1":"0","formula2":"100","errorStyle":"warning"}));
        run(&e, "home.insertSheet", json!({"name":"Other"}));
        let revision = e.latest().active().unwrap().revision;
        assert!(e.execute("__joysheet.validatedSet", json!({"sheet":0,"cell":"A1","input":"999"})).unwrap().result.is_err());
        assert_eq!(revision, e.latest().active().unwrap().revision);
        assert!(e.latest().active().unwrap().workbook.sheets[0].cells.is_empty());
        run(&e, "__joysheet.validatedSet", json!({"sheet":0,"cell":"A1","input":"999","validationAccepted":true}));
        let view = e.latest();
        let wb = &view.active().unwrap().workbook;
        assert_eq!(wb.sheets[0].value(joysheet_engine::core::CellRef::new(0, 0)).display(), "999");
        assert!(wb.sheets[1].cells.is_empty());
        run(&e, "edit.undo", json!({}));
        assert!(e.latest().active().unwrap().workbook.sheets[0].cells.is_empty());
    }
    #[test]
    fn object_batch_is_one_undo_step_and_rolls_back_partial_failure() {
        let e = EngineClient::start().unwrap();
        run(&e, "file.new", json!({}));
        let a = run(&e, "insert.shape", json!({"at":"A1"})).result.unwrap()["shape"].as_u64().unwrap();
        let b = run(&e, "insert.shape", json!({"at":"D1"})).result.unwrap()["shape"].as_u64().unwrap();
        let before = e.latest();
        let undo = before.active().unwrap().undo.len();
        let op = |id, dx| json!({"command":"object.move","params":{"kind":"shape","id":id,"dx":dx}});
        assert!(e.execute("__joysheet.objectBatch", json!({"sheet":0,"operations":[op(a,30),op(999999,50)]})).unwrap().result.is_err());
        assert_eq!(e.latest().active().unwrap().workbook.sheets[0].shapes[0].anchor.dx, 0.);
        assert_eq!(e.latest().active().unwrap().undo.len(), undo);
        run(&e, "__joysheet.objectBatch", json!({"sheet":0,"operations":[op(a,30),op(b,50)]}));
        assert_eq!(e.latest().active().unwrap().undo.len(), undo + 1);
        run(&e, "edit.undo", json!({}));
        assert!(e.latest().active().unwrap().workbook.sheets[0].shapes.iter().all(|s| s.anchor.dx == 0.));
        assert!(e.execute("__joysheet.objectBatch", json!({"sheet":1,"operations":[op(a,10)]})).unwrap().result.is_err());
    }
    #[test]
    fn stale_native_activation_cannot_override_a_new_document() {
        let e = EngineClient::start().unwrap();
        run(&e, "file.new", json!({}));
        let first = e.latest().active_window.unwrap();
        run(&e, "file.new", json!({}));
        let second = e.latest().active_window.unwrap();
        assert_eq!(run(&e, "__joysheet.nativeActivate", json!({"window":first,"previousActive":first})).result.unwrap()["ignored"], true);
        assert_eq!(e.latest().active_window, Some(second));
        run(&e, "__joysheet.nativeActivate", json!({"window":first,"previousActive":second}));
        assert_eq!(e.latest().active_window, Some(first));
        run(&e, "cell.set", json!({"cell":"A1","input":"intended document"}));
        assert_eq!(e.latest().active().unwrap().workbook.sheets[0].value(joysheet_engine::core::CellRef::new(0, 0)).display(), "intended document");
        assert!(e.latest().window(second).unwrap().workbook.sheets[0].cells.is_empty());
    }
    #[test]
    fn preview_import_does_not_mutate_the_live_document() {
        let e = EngineClient::start().unwrap();
        run(&e, "file.new", json!({}));
        let before = e.latest().active().unwrap().workbook.clone();
        let result = run(&e, "__joysheet.previewImport", json!({"text":"Name,Amount\nA,42","sheet":"Preview"})).result.unwrap();
        assert_eq!(result["previewOnly"], true);
        assert_eq!(result["rows"], 2);
        assert!(Arc::ptr_eq(&before, &e.latest().active().unwrap().workbook));
    }
    #[test]
    fn failed_save_preserves_document_and_existing_files() {
        let dir = std::env::temp_dir().join(format!("joysheet-save-failure-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("book.xlsx");
        let temporary = dir.join("book.xlsx.sctmp");
        std::fs::write(&temporary, b"occupied").unwrap();
        let engine = EngineClient::start().unwrap();
        run(&engine, "file.new", json!({}));
        run(&engine, "cell.set", json!({"input":"中文未保存"}));
        assert!(engine.execute("file.save", json!({"path":path})).unwrap().result.is_err());
        assert_eq!(std::fs::read(&temporary).unwrap(), b"occupied");
        assert!(!path.exists());
        assert!(engine.latest().active().unwrap().dirty);
        assert_eq!(run(&engine, "cell.get", json!({})).result.unwrap()["value"], "中文未保存");
        std::fs::remove_file(temporary).unwrap();
        run(&engine, "file.save", json!({"path":path}));
        assert!(!engine.latest().active().unwrap().dirty);
        std::fs::remove_file(path).unwrap();
        std::fs::remove_dir(dir).unwrap();
    }
}
