//! Headless dispatcher adapted from JoyOffice automation: roots, revision checks and replay ledger.
use crate::{Cancellation, editor::Editor, helpers::Scratch, output, show, storage};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    thread::JoinHandle,
};

pub const VERSION: &str = "joydeck-control-1";
pub const LIMIT: usize = 1 << 20;
pub const METHODS: &[&str] = &[
    "engine.commands",
    "document.new",
    "document.open",
    "document.inspect",
    "document.status",
    "document.close",
    "command.execute",
    "document.save",
    "document.exchange",
    "export.start",
    "jobs.get",
    "jobs.cancel",
    "show.start",
    "show.action",
    "show.tick",
    "show.status",
    "show.frame",
];
#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Request {
    pub protocol_version: String,
    pub request_id: String,
    pub method: String,
    pub target: Option<u64>,
    pub expected_revision: Option<u64>,
    #[serde(default)]
    pub params: Value,
}
struct Job {
    cancel: Cancellation,
    status: Arc<Mutex<Value>>,
    thread: Option<JoinHandle<()>>,
}
impl Drop for Job {
    fn drop(&mut self) {
        self.cancel.cancel();
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}
pub struct Dispatcher {
    root: PathBuf,
    editor: Option<Editor>,
    show: Option<show::Player>,
    document_id: u64,
    revision: u64,
    jobs: BTreeMap<u64, Job>,
    next_job: u64,
    completed: BTreeMap<String, ([u8; 32], Value)>,
    completed_bytes: usize,
    _scratch: Scratch,
}
impl Dispatcher {
    pub fn new(root: &Path) -> Result<Self, String> {
        let root = root.canonicalize().map_err(|e| e.to_string())?;
        if !root.is_dir() {
            return Err("root must be an existing directory".into());
        }
        Ok(Self {
            root,
            editor: None,
            show: None,
            document_id: 0,
            revision: 0,
            jobs: BTreeMap::new(),
            next_job: 1,
            completed: BTreeMap::new(),
            completed_bytes: 0,
            _scratch: Scratch::new()?,
        })
    }
    pub fn path(&self, value: &str, write: bool) -> Result<PathBuf, String> {
        use std::path::Component;
        let p = Path::new(value);
        if !p.is_absolute()
            || p.components().any(|c| {
                matches!(c, Component::ParentDir)
                    || matches!(c,Component::Normal(s) if s.to_string_lossy().contains([':', '\0']))
            })
        {
            return Err("absolute path without traversal/ADS required".into());
        }
        let real = if write {
            if p.symlink_metadata().is_ok() {
                return Err("output must be new".into());
            }
            p.parent()
                .ok_or("output parent missing")?
                .canonicalize()
                .map_err(|e| e.to_string())?
                .join(p.file_name().ok_or("output filename missing")?)
        } else {
            p.canonicalize().map_err(|e| e.to_string())?
        };
        if !real.starts_with(&self.root) {
            return Err("path outside authorized root".into());
        }
        Ok(real)
    }
    fn cancel_jobs(&mut self) {
        for job in self.jobs.values() {
            job.cancel.cancel();
        }
    }
    fn status(&self) -> Value {
        if let Some(e) = &self.editor {
            json!({"documentId":self.document_id,"revision":self.revision,"engineRevision":e.session.revision(),"dirty":e.session.dirty(),"pages":e.session.model().slides.len(),"selection":e.session.selection()})
        } else {
            json!({"documentId":self.document_id,"open":false})
        }
    }
    pub fn dispatch(&mut self, r: Request) -> Value {
        let fail = |code: &str, message: String| json!({"requestId":r.request_id,"error":{"code":code,"message":message}});
        let Ok(bytes) = serde_json::to_vec(&r) else {
            return fail("invalid-request", "invalid JSON".into());
        };
        if r.protocol_version != VERSION
            || r.request_id.is_empty()
            || r.request_id.len() > 128
            || bytes.len() > LIMIT
        {
            return fail(
                "invalid-request",
                "protocol/requestId/1 MiB request limit".into(),
            );
        }
        let digest: [u8; 32] = Sha256::digest(bytes).into();
        if let Some((old, response)) = self.completed.get(&r.request_id) {
            return if old == &digest {
                response.clone()
            } else {
                fail(
                    "id-conflict",
                    "requestId reused with different arguments".into(),
                )
            };
        }
        let read = matches!(
            r.method.as_str(),
            "engine.commands" | "document.inspect" | "document.status" | "jobs.get" | "show.status"
        );
        if !read && (self.completed.len() >= 128 || self.completed_bytes > 12 << 20) {
            return fail(
                "limit",
                "mutation replay ledger full; start a new stdio session".into(),
            );
        }
        let response = match self.execute(&r) {
            Ok(result) => json!({"requestId":r.request_id,"result":result}),
            Err(e) => fail("operation-failed", e),
        };
        let response = if serde_json::to_vec(&response).map_or(usize::MAX, |b| b.len()) > 4 << 20 {
            fail(
                "limit",
                "response exceeds 4 MiB; narrow the query or export".into(),
            )
        } else {
            response
        };
        if !read {
            self.completed_bytes += serde_json::to_vec(&response).map_or(0, |b| b.len());
            self.completed
                .insert(r.request_id, (digest, response.clone()));
        }
        response
    }
    fn execute(&mut self, r: &Request) -> Result<Value, String> {
        let p = &r.params;
        let string = |k: &str| p[k].as_str().ok_or_else(|| format!("missing string {k}"));
        if r.method == "engine.commands" {
            return Ok(
                json!({"protocolVersion":VERSION,"methods":METHODS,"commands":joydeck_engine::COMMANDS,"documentId":self.document_id,"transport":"headless stdio","root":self.root,"outputs":"new paths only","replay":"128 mutations / 16 MiB, same requestId and payload","attachedDesktop":false}),
            );
        }
        if r.method == "document.status" {
            return Ok(self.status());
        }
        if matches!(r.method.as_str(), "jobs.get" | "jobs.cancel") {
            let id = p["jobId"].as_u64().ok_or("missing jobId")?;
            let job = self.jobs.get_mut(&id).ok_or("job missing")?;
            let mut status = job.status.lock().map_err(|_| "job status unavailable")?;
            if r.method == "jobs.cancel" && status["state"] == "running" {
                job.cancel.cancel();
                status["cancelRequested"] = json!(true);
            }
            return Ok(status.clone());
        }
        if matches!(r.method.as_str(), "document.new" | "document.open") {
            if self.editor.as_ref().is_some_and(|e| e.session.dirty())
                && p["dirtyPolicy"] != "discard"
            {
                return Err("unsaved document; explicit dirtyPolicy=discard required".into());
            }
            let editor = if r.method == "document.open" {
                Editor::open(
                    &self.path(string("path")?, false)?,
                    self._scratch.0.join("recovery"),
                )?
            } else {
                Editor::new(self._scratch.0.join("recovery"))?
            };
            let mut nonce = [0u8; 8];
            getrandom::fill(&mut nonce).map_err(|e| e.to_string())?;
            self.cancel_jobs();
            self.show = None;
            self.editor = Some(editor);
            self.document_id = (u64::from_le_bytes(nonce) & ((1 << 53) - 1)).max(1);
            self.revision = 0;
            return Ok(self.status());
        }
        if r.target != Some(self.document_id) || self.editor.is_none() {
            return Err("target must match current documentId".into());
        }
        let mutating = matches!(
            r.method.as_str(),
            "command.execute" | "document.save" | "document.close" | "document.exchange"
        );
        if mutating && r.expected_revision != Some(self.revision) {
            return Err("expectedRevision missing/stale".into());
        }
        match r.method.as_str() {
            "document.inspect" => {
                let doc = self.editor.as_ref().ok_or("no document")?.display()?;
                serde_json::to_value(doc.info).map_err(|e| e.to_string())
            }
            "command.execute" => {
                let command =
                    serde_json::from_value::<joydeck_engine::Command>(p["command"].clone())
                        .map_err(|e| e.to_string())?;
                self.editor
                    .as_mut()
                    .ok_or("no document")?
                    .session
                    .execute(command)?;
                self.revision += 1;
                Ok(self.status())
            }
            "document.save" => {
                let path = self.path(string("path")?, true)?;
                let editor = self.editor.as_mut().ok_or("no document")?;
                let saved = editor.save_job(path)?.run()?;
                editor.apply_saved(saved);
                self.revision += 1;
                Ok(self.status())
            }
            "document.exchange" => {
                let path = self.path(string("path")?, true)?;
                crate::exchange::export(
                    self.editor.as_ref().ok_or("no document")?,
                    &path,
                    string("mode")?,
                    &Cancellation::default(),
                )
            }
            "document.close" => {
                if self.editor.as_ref().is_some_and(|e| e.session.dirty())
                    && p["dirtyPolicy"] != "discard"
                {
                    return Err("unsaved document; dirtyPolicy=discard required".into());
                }
                self.cancel_jobs();
                self.editor = None;
                self.show = None;
                Ok(self.status())
            }
            "export.start" => {
                let path = self.path(string("path")?, true)?;
                let opts: output::Options =
                    serde_json::from_value(p["options"].clone()).map_err(|e| e.to_string())?;
                let doc = self.editor.as_ref().ok_or("no document")?.display()?;
                output::pages(&doc, &opts)?;
                if self.jobs.len() >= 16 {
                    return Err("16 retained jobs limit; start a new session".into());
                }
                let running = self
                    .jobs
                    .values()
                    .filter(|j| j.status.lock().is_ok_and(|s| s["state"] == "running"))
                    .count();
                if running >= 2 {
                    return Err("two active export jobs limit".into());
                }
                let id = self.next_job;
                self.next_job += 1;
                let cancel = Cancellation::default();
                let token = cancel.clone();
                let status = Arc::new(Mutex::new(
                    json!({"jobId":id,"documentId":self.document_id,"revision":self.revision,"state":"running","phase":"preparing","cancelRequested":false}),
                ));
                let state = status.clone();
                let thread = std::thread::spawn(move || {
                    if let Ok(mut s) = state.lock() {
                        s["phase"] = json!("encoding");
                    }
                    let result = output::export(&doc, &path, &opts, &token);
                    if let Ok(mut s) = state.lock() {
                        match result {
                            Ok(report) => {
                                s["state"] = json!("completed");
                                s["result"] = serde_json::to_value(report).unwrap_or(Value::Null);
                            }
                            Err(e) => {
                                s["state"] = json!(if token.check().is_err() {
                                    "cancelled"
                                } else {
                                    "failed"
                                });
                                s["error"] = json!(e);
                            }
                        }
                        s["phase"] = json!("finished");
                    }
                });
                self.jobs.insert(
                    id,
                    Job {
                        cancel,
                        status,
                        thread: Some(thread),
                    },
                );
                Ok(
                    json!({"jobId":id,"state":"running","semantics":"cancel before publication prevents output; commit may win a concurrent cancellation"}),
                )
            }
            "show.start" => {
                let e = self.editor.as_ref().ok_or("no document")?;
                let page = p["page"]
                    .as_u64()
                    .map(|n| {
                        usize::try_from(n)
                            .ok()
                            .and_then(|n| n.checked_sub(1))
                            .ok_or("invalid show page")
                    })
                    .transpose()?;
                self.show = Some(show::Player::new(
                    e.session.snapshot().model,
                    page,
                    p["custom"].as_str(),
                )?);
                Ok(self
                    .show
                    .as_ref()
                    .map(|s| s.status())
                    .unwrap_or(Value::Null))
            }
            "show.action" => {
                let show = self.show.as_mut().ok_or("show not started")?;
                let page = p["page"]
                    .as_u64()
                    .and_then(|n| usize::try_from(n).ok())
                    .and_then(|n| n.checked_sub(1));
                let exit = show.navigate(string("action")?, page)?;
                Ok(json!({"exit":exit,"status":show.status()}))
            }
            "show.tick" => {
                let show = self.show.as_mut().ok_or("show not started")?;
                show.advance(p["seconds"].as_f64().ok_or("seconds missing")?)?;
                Ok(show.status())
            }
            "show.status" => Ok(self.show.as_ref().ok_or("show not started")?.status()),
            "show.frame" => {
                let path = self.path(string("path")?, true)?;
                let show = self.show.as_ref().ok_or("show not started")?;
                let image =
                    show.frame(p["scale"].as_f64().unwrap_or(1.), &Cancellation::default())?;
                let bytes = image.to_png();
                if bytes.is_empty() {
                    return Err("PNG encoder failure".into());
                }
                storage::save(&path, &bytes, None)?;
                Ok(json!({"path":path,"bytes":bytes.len(),"state":show.status()}))
            }
            _ => Err("unknown control method".into()),
        }
    }
}
impl Drop for Dispatcher {
    fn drop(&mut self) {
        self.cancel_jobs();
        self.jobs.clear();
    }
}
