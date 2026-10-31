//! Opt-in authenticated loopback control. At most eight synchronous requests can wait/run.
use crate::{
    automation::{Dispatcher, LIMIT, Request},
    document::Result,
};
use serde_json::json;
use std::{
    io::{BufRead, BufReader, Read, Write},
    net::{SocketAddr, TcpListener, TcpStream},
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    time::Duration,
};
pub struct Service {
    pub document_id: u64,
    pub address: SocketAddr,
    pub descriptor: PathBuf,
    stop: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}
fn send(s: &mut TcpStream, value: serde_json::Value) {
    let _ = writeln!(s, "{value}");
}
impl Service {
    pub fn start(dispatcher: Dispatcher, descriptor: &Path, attached: bool) -> Result<Self> {
        if !cfg!(unix) {
            return Err("此版本本地控制凭据仅支持Unix权限；Windows可用headless stdio".into());
        }
        let listener = TcpListener::bind("127.0.0.1:0").map_err(|e| e.to_string())?;
        listener.set_nonblocking(true).map_err(|e| e.to_string())?;
        let address = listener.local_addr().map_err(|e| e.to_string())?;
        let mut random = [0u8; 32];
        getrandom::fill(&mut random).map_err(|e| e.to_string())?;
        let token = random
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>();
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(descriptor).map_err(|e| e.to_string())?;
        writeln!(file,"{}",json!({"address":address.to_string(),"token":token,"protocolVersion":crate::automation::VERSION,"documentId":dispatcher.session.id,"semantics":"serial synchronous; disconnect while running means outcome-unknown; retry only same requestId"})).map_err(|e|e.to_string())?;
        let document_id = dispatcher.session.id;
        let dispatcher = Arc::new(Mutex::new(dispatcher));
        let stop = Arc::new(AtomicBool::new(false));
        let shutdown = stop.clone();
        let clients = Arc::new(AtomicUsize::new(0));
        let thread = std::thread::spawn(move || {
            while !shutdown.load(Ordering::Relaxed) {
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        if clients.fetch_add(1, Ordering::SeqCst) >= 8 {
                            clients.fetch_sub(1, Ordering::SeqCst);
                            send(&mut stream, json!({"error":{"code":"busy"}}));
                            continue;
                        }
                        let clients = clients.clone();
                        let dispatcher = dispatcher.clone();
                        let token = token.clone();
                        let shutdown = shutdown.clone();
                        std::thread::spawn(move || {
                            let _ = stream.set_nonblocking(false);
                            let _ = stream.set_read_timeout(Some(Duration::from_secs(3)));
                            let _ = stream.set_write_timeout(Some(Duration::from_secs(3)));
                            let mut bytes = vec![];
                            if BufReader::new(&mut stream)
                                .take((LIMIT + 1) as u64)
                                .read_until(b'\n', &mut bytes)
                                .is_err()
                                || bytes.len() > LIMIT
                            {
                                send(
                                    &mut stream,
                                    json!({"error":{"code":"invalid-request","message":"invalid framing or JSON request"}}),
                                );
                            } else if let Ok(v) =
                                serde_json::from_slice::<serde_json::Value>(&bytes)
                            {
                                // Compare the complete fixed-size token without early exit on differing content.
                                let supplied = v["token"].as_str().unwrap_or("").as_bytes();
                                let valid = supplied.len() == token.len()
                                    && supplied
                                        .iter()
                                        .zip(token.as_bytes())
                                        .fold(0u8, |acc, (a, b)| acc | (a ^ b))
                                        == 0;
                                if !valid {
                                    send(&mut stream, json!({"error":{"code":"unauthenticated"}}));
                                } else if let Ok(r) =
                                    serde_json::from_value::<Request>(v["request"].clone())
                                {
                                    if shutdown.load(Ordering::Relaxed) {
                                        send(
                                            &mut stream,
                                            json!({"error":{"code":"queued-cancelled"}}),
                                        );
                                    } else if attached
                                        && matches!(
                                            r.method.as_str(),
                                            "document.new" | "document.open"
                                        )
                                    {
                                        send(
                                            &mut stream,
                                            json!({"error":{"code":"unsupported","message":"Attached desktop cannot replace its document remotely"}}),
                                        );
                                    } else {
                                        let Ok(mut d) = dispatcher.lock() else {
                                            send(
                                                &mut stream,
                                                json!({"error":{"code":"session-failed","message":"文档会话已损坏"}}),
                                            );
                                            continue;
                                        };
                                        let response = if shutdown.load(Ordering::Relaxed) {
                                            json!({"error":{"code":"queued-cancelled"}})
                                        } else {
                                            d.dispatch(r)
                                        };
                                        send(&mut stream, response);
                                    }
                                } else {
                                    let diagnostic =
                                        serde_json::from_value::<Request>(v["request"].clone())
                                            .err()
                                            .map(|e| e.to_string());
                                    send(
                                        &mut stream,
                                        json!({"error":{"code":"invalid-request","message":diagnostic}}),
                                    );
                                }
                            } else {
                                send(
                                    &mut stream,
                                    json!({"error":{"code":"invalid-request","message":serde_json::from_slice::<serde_json::Value>(&bytes).err().map(|e|e.to_string()).unwrap_or_else(||"无效请求".into()),"length":bytes.len()}}),
                                );
                            }
                            clients.fetch_sub(1, Ordering::SeqCst);
                        });
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(Duration::from_millis(20))
                    }
                    Err(_) => break,
                }
            }
        });
        Ok(Self {
            document_id,
            address,
            descriptor: descriptor.to_owned(),
            stop,
            thread: Some(thread),
        })
    }
}
impl Drop for Service {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
        let _ = std::fs::remove_file(&self.descriptor);
    }
}
/// A network failure after writing a request cannot prove that a mutation did not run.
pub fn remote(descriptor: &Path, request: &Request) -> Result<serde_json::Value> {
    let meta = std::fs::metadata(descriptor).map_err(|e| e.to_string())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if meta.permissions().mode() & 0o077 != 0 {
            return Err("控制凭据必须仅当前用户可读写".into());
        }
    }
    if meta.len() > 4096 {
        return Err("凭据描述超限".into());
    }
    let v: serde_json::Value =
        serde_json::from_slice(&std::fs::read(descriptor).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
    let address: SocketAddr = v["address"]
        .as_str()
        .ok_or("缺少address")?
        .parse::<SocketAddr>()
        .map_err(|e| e.to_string())?;
    if !address.ip().is_loopback() {
        return Err("仅可连接loopback".into());
    }
    let token = v["token"].as_str().ok_or("缺少token")?;
    let mut stream =
        TcpStream::connect_timeout(&address, Duration::from_secs(3)).map_err(|e| e.to_string())?;
    stream
        .set_read_timeout(Some(Duration::from_secs(30)))
        .map_err(|e| e.to_string())?;
    writeln!(stream, "{}", json!({"token":token,"request":request}))
        .map_err(|_| "outcome-unknown：写入失败；仅以相同requestId重试".to_string())?;
    let mut response = vec![];
    BufReader::new(stream)
        .take(16 * 1024 * 1024)
        .read_until(b'\n', &mut response)
        .map_err(|_| "outcome-unknown：响应超时；仅以相同requestId重试".to_string())?;
    serde_json::from_slice(&response)
        .map_err(|_| "outcome-unknown：响应不完整；仅以相同requestId重试".into())
}
