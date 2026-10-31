//! Authenticated loopback transport. UI work is handed to the GPUI thread.
use crate::EngineClient;
use joysheet_protocol::{ControlRequest, MAX_LINE_BYTES, response};
use serde_json::Value;
use std::{
    collections::VecDeque,
    io::{BufRead, BufReader, Read, Write},
    net::{TcpListener, TcpStream},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU8, AtomicUsize, Ordering},
        mpsc,
    },
    time::Duration,
};

pub struct UiCall {
    pub method: String,
    pub params: Value,
    pub window: Option<u64>,
    pub reply: mpsc::Sender<Result<Value, String>>,
    pub state: Arc<AtomicU8>,
}
impl UiCall {
    pub fn claim(&self) -> bool {
        self.state.compare_exchange(0, 1, Ordering::AcqRel, Ordering::Acquire).is_ok()
    }
    pub fn respond(self, result: Result<Value, String>) {
        let _ = self.reply.send(result);
    }
}
#[derive(Clone, Default)]
pub struct UiQueue(Arc<Mutex<VecDeque<UiCall>>>);
impl UiQueue {
    pub fn take(&self, window: Option<u64>) -> Option<UiCall> {
        let mut queue = self.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        let index = queue.iter().position(|call| call.window == window || call.window.is_none())?;
        queue.remove(index)
    }
    fn send(&self, call: UiCall) -> Result<(), String> {
        let mut queue = self.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        if queue.len() >= 64 {
            return Err("UI control queue is full".into());
        }
        queue.push_back(call);
        Ok(())
    }
}
pub struct ControlServer {
    pub port: u16,
    pub queue: UiQueue,
    stop: Arc<AtomicBool>,
}
impl ControlServer {
    pub fn start(port: u16, engine: EngineClient) -> Result<Self, String> {
        let listener = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, port)).map_err(|e| format!("control listener: {e}"))?;
        let port = listener.local_addr().map_err(|e| e.to_string())?.port();
        listener.set_nonblocking(true).map_err(|e| e.to_string())?;
        let token = if let Some(token) = ["JOYSHEET_CONTROL_TOKEN", "GRIDCRAFT_CONTROL_TOKEN"].iter().find_map(|name| std::env::var(name).ok()) {
            if token.trim().len() < 16 {
                return Err("control token must have at least 16 characters".into());
            }
            token.trim().to_string()
        } else {
            let mut bytes = [0u8; 32];
            getrandom::fill(&mut bytes).map_err(|e| e.to_string())?;
            let token = gridcraft_mcp::control_auth::hex(&bytes);
            let path = joysheet_protocol::token_path(port);
            let parent = path.parent().ok_or("token folder unavailable")?;
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(parent, std::fs::Permissions::from_mode(0o700)).map_err(|e| e.to_string())?;
            }
            let mut options = std::fs::OpenOptions::new();
            options.write(true).create(true).truncate(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                options.mode(0o600);
            }
            options.open(&path).and_then(|mut f| f.write_all(token.as_bytes())).map_err(|e| e.to_string())?;
            eprintln!("Joysheet control: 127.0.0.1:{port}; token: {}", path.display());
            token
        };
        let queue = UiQueue::default();
        let inbox = queue.clone();
        let stop = Arc::new(AtomicBool::new(false));
        let stopped = stop.clone();
        let connections = Arc::new(AtomicUsize::new(0));
        std::thread::Builder::new()
            .name("joysheet-control".into())
            .spawn(move || {
                while !stopped.load(Ordering::Acquire) {
                    match listener.accept() {
                        Ok((stream, _)) => {
                            if connections.fetch_add(1, Ordering::AcqRel) >= 8 {
                                connections.fetch_sub(1, Ordering::AcqRel);
                                continue;
                            }
                            let active = connections.clone();
                            let token = token.clone();
                            let engine = engine.clone();
                            let queue = inbox.clone();
                            let started = std::thread::Builder::new().name("joysheet-control-peer".into()).spawn(move || {
                                if let Err(error) = serve(stream, &token, &engine, &queue) {
                                    eprintln!("Joysheet control peer: {error}");
                                }
                                active.fetch_sub(1, Ordering::AcqRel);
                            });
                            if started.is_err() {
                                connections.fetch_sub(1, Ordering::AcqRel);
                            }
                        }
                        Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => std::thread::sleep(Duration::from_millis(15)),
                        Err(_) => break,
                    }
                }
            })
            .map_err(|e| e.to_string())?;
        Ok(Self { port, queue, stop })
    }
}
impl Drop for ControlServer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
    }
}
fn read_line(reader: &mut BufReader<TcpStream>, limit: usize) -> Result<Option<String>, String> {
    let mut line = String::new();
    let bytes = reader.take(limit as u64 + 1).read_line(&mut line).map_err(|e| e.to_string())?;
    if bytes == 0 {
        return Ok(None);
    }
    if bytes > limit || !line.ends_with('\n') {
        return Err("control line exceeds its bound or is incomplete".into());
    }
    Ok(Some(line))
}
fn write_line(stream: &mut TcpStream, value: Value) -> Result<(), String> {
    serde_json::to_writer(&mut *stream, &value).map_err(|e| e.to_string())?;
    stream.write_all(b"\n").and_then(|_| stream.flush()).map_err(|e| e.to_string())
}
fn serve(stream: TcpStream, token: &str, engine: &EngineClient, queue: &UiQueue) -> Result<(), String> {
    // On Windows accepted sockets inherit the listener's nonblocking mode.
    stream.set_nonblocking(false).map_err(|e| e.to_string())?;
    stream.set_read_timeout(Some(Duration::from_secs(30))).map_err(|e| e.to_string())?;
    stream.set_write_timeout(Some(Duration::from_secs(30))).map_err(|e| e.to_string())?;
    let mut writer = stream.try_clone().map_err(|e| e.to_string())?;
    let mut reader = BufReader::new(stream);
    let Some(first) = read_line(&mut reader, 4096)? else {
        return Ok(());
    };
    match gridcraft_mcp::control_auth::check_first_line(&first, token) {
        Ok(reply) => write_line(&mut writer, reply)?,
        Err(reply) => {
            write_line(&mut writer, reply)?;
            return Ok(());
        }
    }
    while let Some(line) = read_line(&mut reader, MAX_LINE_BYTES)? {
        let mut request = match ControlRequest::parse(&line) {
            Ok(request) => request,
            Err(error) => {
                write_line(&mut writer, response(Value::Null, Err(error)))?;
                continue;
            }
        };
        if request.method == "engine.execute"
            && let Some(command) = request.params.get("command").and_then(Value::as_str)
        {
            let alias = match command {
                "file.save" | "file.saveAs" => Some("app.save"),
                "file.open" => Some("app.open"),
                "file.new" => Some("app.new"),
                _ => None,
            };
            let alias = alias.filter(|_| {
                command != "file.saveAs"
                    || request.params.get("params").and_then(|p| p.get("path")).and_then(Value::as_str).is_some_and(|s| !s.is_empty())
            });
            if let Some(alias) = alias {
                let mut params = request.params.get("params").cloned().unwrap_or_else(|| serde_json::json!({}));
                if let Some(m) = params.as_object_mut() {
                    for key in ["window", "doc"] {
                        if let Some(value) = request.params.get(key) {
                            m.insert(key.into(), value.clone());
                        }
                    }
                }
                request.method = alias.into();
                request.params = params;
            }
        }
        let result = (|| -> Result<Value, String> {
            if request.method.starts_with("ui.") || matches!(request.method.as_str(), "app.quit" | "app.open" | "app.save" | "app.new" | "command") {
                let target = joysheet_protocol::CommandTarget::from_params(&request.params)?;
                let snapshot = engine.latest();
                let window = match (target.window, target.doc) {
                    (Some(id), None) => Some(id),
                    (None, Some(doc)) => snapshot.windows.iter().find(|w| w.doc == doc).map(|w| w.window),
                    (None, None) => snapshot.active_window,
                    _ => return Err("specify either window or doc".into()),
                };
                if window.is_none() || window.is_some_and(|id| snapshot.window(id).is_none()) {
                    Err("target window is unavailable".into())
                } else {
                    let (reply, receiver) = mpsc::channel();
                    let state = Arc::new(AtomicU8::new(0));
                    let budget = Duration::from_secs(if request.method.starts_with("app.") { 60 } else { 8 });
                    queue.send(UiCall { method: request.method, params: request.params, window, reply, state: state.clone() })?;
                    match receiver.recv_timeout(budget) {
                        Ok(result) => result,
                        Err(_) => {
                            let _ = state.compare_exchange(0, 2, Ordering::AcqRel, Ordering::Acquire);
                            Err("UI request timed out".into())
                        }
                    }
                }
            } else {
                engine.dispatch(&request.method, request.params)
            }
        })();
        write_line(&mut writer, response(request.id, result))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn auth_and_remote_commands() {
        let engine = EngineClient::start().unwrap();
        engine.execute("file.new", json!({})).unwrap().result.unwrap();
        let server = ControlServer::start(0, engine).unwrap();
        let addr = format!("127.0.0.1:{}", server.port);
        let token = joysheet_protocol::client_token(&addr).unwrap();
        let mut remote = gridcraft_mcp::Remote::connect_with_token(&addr, token).unwrap();
        use gridcraft_mcp::Backend;
        remote.call("engine.execute", json!({"command":"cell.set","params":{"input":"=SUM(1,2,3)"}})).unwrap();
        assert_eq!(remote.call("cell.get", json!({})).unwrap()["value"], 6.0);
        assert!(gridcraft_mcp::Remote::connect_with_token(&addr, "incorrect-token-123".into()).is_err());
        assert!(remote.call("ui.inspect", json!({"doc":999999})).is_err());
        assert!(remote.call("ui.inspect", json!({"doc":1,"window":1})).is_err());
        assert_eq!(remote.call("cell.get", json!({})).unwrap()["value"], 6.0);
    }
    #[test]
    fn timed_out_pending_calls_are_not_run() {
        let state = Arc::new(AtomicU8::new(2));
        let (reply, _) = mpsc::channel();
        let call = UiCall { method: "ui.text".into(), params: json!({}), window: Some(1), reply, state };
        assert!(!call.claim());
    }
}
