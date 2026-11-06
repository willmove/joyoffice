//! Opt-in, authenticated loopback control over the live desktop session.
use super::*;
use gpui::component::WindowExt;
use serde_json::{Value, json};
use std::{
    io::{BufRead, BufReader, Read, Write},
    net::{Ipv4Addr, TcpListener, TcpStream},
    sync::mpsc,
    time::{Duration, Instant},
};

const COMMANDS: &[&str] = &[
    "text.insert",
    "edit.undo",
    "edit.redo",
    "select.all",
    "caret.docStart",
    "caret.docEnd",
    "edit.goto",
    "view.zoom",
];
const VIEW_COMMANDS: &[&str] = &["edit.goto", "view.zoom"];
const METHODS: &[&str] = &[
    "capabilities",
    "engine.commands",
    "document.inspect",
    "engine.execute",
    "command",
];
const MAX_LINE: usize = 64 << 10;

struct Pending {
    request: Value,
    expires: Instant,
    reply: mpsc::SyncSender<Value>,
}

pub(super) fn install(port: u16, view: &gpui::Entity<JoyWord>, w: &Window, cx: &mut App) {
    let (tx, rx) = async_channel::bounded(1);
    if let Err(error) = start(port, tx) {
        view.update(cx, |v, cx| {
            v.shell.notice = Some(format!("Control: {error}"));
            cx.notify();
        });
        return;
    }
    let handle = w.window_handle();
    let view = view.downgrade();
    cx.spawn(async move |cx| {
        while let Ok(pending) = rx.recv().await {
            let id = pending.request["id"].clone();
            let result = if Instant::now() >= pending.expires {
                Err("control request expired".into())
            } else {
                cx.update(|cx| {
                    handle
                        .update(cx, |_, w, cx| {
                            view.update(cx, |v, cx| v.control_request(&pending.request, w, cx))
                                .map_err(|_| "window closed".to_string())?
                        })
                        .map_err(|_| "window closed".to_string())?
                })
            };
            let _ = pending.reply.send(reply(id, result));
            if view.upgrade().is_none() {
                break;
            }
        }
    })
    .detach();
}

fn start(port: u16, tx: async_channel::Sender<Pending>) -> Result<(), String> {
    let key = std::env::var("JOYWORD_CONTROL_KEY")
        .map_err(|_| "set JOYWORD_CONTROL_KEY before enabling --control")?;
    let key = key.trim().to_owned();
    if !(32..=128).contains(&key.len()) || !key.bytes().all(|c| c.is_ascii_graphic()) {
        return Err("JOYWORD_CONTROL_KEY needs 32–128 printable ASCII characters".into());
    }
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, port)).map_err(|e| e.to_string())?;
    listener.set_nonblocking(true).map_err(|e| e.to_string())?;
    std::thread::Builder::new()
        .name("joyword-control".into())
        .spawn(move || {
            while !tx.is_closed() {
                match listener.accept() {
                    Ok((stream, _)) => {
                        let _ = connection(stream, &key, &tx);
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(Duration::from_millis(25))
                    }
                    Err(_) => break,
                }
            }
        })
        .map_err(|e| e.to_string())?;
    Ok(())
}

fn reply(id: Value, result: Result<Value, String>) -> Value {
    match result {
        Ok(value) => json!({"id":id,"ok":true,"result":value}),
        Err(error) => json!({"id":id,"ok":false,"error":error}),
    }
}

struct TimedRead {
    stream: TcpStream,
    deadline: Instant,
}

impl Read for TimedRead {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let remaining = self.deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err(std::io::ErrorKind::TimedOut.into());
        }
        self.stream.set_read_timeout(Some(remaining))?;
        self.stream.read(buf)
    }
}

fn connection(
    mut stream: TcpStream,
    key: &str,
    tx: &async_channel::Sender<Pending>,
) -> std::io::Result<()> {
    stream.set_write_timeout(Some(Duration::from_secs(2)))?;
    let mut line = Vec::new();
    let expiry = Instant::now() + Duration::from_secs(30);
    let mut input = BufReader::new(TimedRead {
        stream: stream.try_clone()?,
        deadline: expiry,
    });
    while Instant::now() < expiry && !tx.is_closed() {
        input.get_mut().deadline = expiry.min(Instant::now() + Duration::from_secs(2));
        line.clear();
        if input
            .by_ref()
            .take(MAX_LINE as u64 + 1)
            .read_until(b'\n', &mut line)?
            == 0
            || line.len() > MAX_LINE
            || line.last() != Some(&b'\n')
        {
            break;
        }
        let request: Value = match serde_json::from_slice(&line) {
            Ok(v) => v,
            Err(_) => break,
        };
        let candidate = request["key"].as_str().unwrap_or_default();
        let authorized = candidate.len() == key.len()
            && candidate
                .bytes()
                .zip(key.bytes())
                .fold(0, |diff, (a, b)| diff | (a ^ b))
                == 0;
        let id = request["id"].clone();
        let result = if authorized {
            let (send, recv) = mpsc::sync_channel(1);
            let expires = Instant::now() + Duration::from_secs(5);
            match tx.try_send(Pending {
                request,
                expires,
                reply: send,
            }) {
                Ok(()) => recv
                    .recv_timeout(Duration::from_secs(6))
                    .unwrap_or_else(|_| reply(id, Err("control request timed out".into()))),
                Err(_) => reply(id, Err("control channel busy".into())),
            }
        } else {
            reply(id, Err("unauthorized".into()))
        };
        serde_json::to_writer(&mut stream, &result)?;
        stream.write_all(b"\n")?;
        if !authorized {
            break;
        }
    }
    Ok(())
}

impl JoyWord {
    fn control_request(
        &mut self,
        request: &Value,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<Value, String> {
        let method = request["method"].as_str().ok_or("missing method")?;
        let p = &request["params"];
        if method == "capabilities" {
            return Ok(
                json!({"methods":METHODS,"commands":COMMANDS,"fileAccess":false,"uiAutomation":true,"uiCommands":VIEW_COMMANDS,"versionRequired":true}),
            );
        }
        if method == "engine.commands" {
            return Ok(json!(
                COMMANDS
                    .iter()
                    .map(|id| json!({"id":id}))
                    .collect::<Vec<_>>()
            ));
        }
        if self.canvas.loading || self.saving || self.close_pending {
            return Err("document is busy".into());
        }
        let reader = self.canvas.reader.as_ref().ok_or("no active document")?;
        let doc = reader.engine().document_id();
        let version = reader.engine().rev();
        if !p["doc"].is_null() && p["doc"].as_u64() != Some(doc) {
            return Err("unknown document".into());
        }
        match method {
            "document.inspect" => {
                let text = reader.body_text();
                if text.len() > 4 << 20 {
                    return Err("control text exceeds 4 MiB".into());
                }
                Ok(json!({"doc":doc,"version":version,"text":text,"summary":reader.summary}))
            }
            "engine.execute" | "command" => {
                let cmd = p["command"].as_str().ok_or("missing command")?;
                if !COMMANDS.contains(&cmd) {
                    return Err("command is outside the desktop control allowlist".into());
                }
                if p["doc"].as_u64().or_else(|| p["params"]["doc"].as_u64()) != Some(doc)
                    || p["version"]
                        .as_u64()
                        .or_else(|| p["params"]["version"].as_u64())
                        != Some(version)
                {
                    return Err("document or version is stale".into());
                }
                if w.has_active_dialog(cx) || reader.is_composing() {
                    return Err("finish the active dialog or composition first".into());
                }
                let args = match cmd {
                    "text.insert" => {
                        json!({"text":p["params"]["text"].as_str().ok_or("missing text")?})
                    }
                    "edit.goto" => {
                        let page = p["params"]["page"]
                            .as_u64()
                            .filter(|page| (1..=reader.summary.pages.len() as u64).contains(page))
                            .ok_or("page must name an existing page")?;
                        json!({"page":page})
                    }
                    "view.zoom" => {
                        let value = p["params"]["value"]
                            .as_f64()
                            .filter(|value| (10.0..=500.0).contains(value))
                            .ok_or("zoom must be 10–500 percent")?;
                        json!({"value":value})
                    }
                    _ => json!({}),
                };
                self.try_edit(cmd, &args, w, cx)
            }
            _ => Err("unsupported control method".into()),
        }
    }
}
