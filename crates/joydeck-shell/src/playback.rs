//! Private stdio media protocol, kill-on-drop ownership and bounded native frame delivery.
use crate::helpers::{self, Scratch};
use base64::Engine;
use serde_json::{Value, json};
use std::{
    io::{BufRead, BufReader, Read, Write},
    process::{Child, ChildStdin, Stdio},
    sync::mpsc::{self, Receiver},
    time::Duration,
};

pub struct Player {
    child: Child,
    input: ChildStdin,
    responses: Receiver<Result<Value, String>>,
    pub info: Value,
    poisoned: bool,
    _scratch: Scratch,
}
impl Player {
    pub fn new(bytes: &[u8], extension: &str) -> Result<Self, String> {
        if bytes.is_empty()
            || bytes.len() > 64 << 20
            || !["mp4", "mov", "wmv", "mp3", "wav", "wma", "m4a"].contains(&extension)
        {
            return Err("unsupported/oversize local media".into());
        }
        let scratch = Scratch::new()?;
        let file = scratch.0.join(format!("media.{extension}"));
        std::fs::write(&file, bytes).map_err(|e| e.to_string())?;
        let mut child = helpers::command(helpers::find("media")?)
            .arg(helpers::native_path(&file)?)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| e.to_string())?;
        let input = child.stdin.take().ok_or("missing media input")?;
        let stdout = child.stdout.take().ok_or("missing media output")?;
        let (tx, responses) = mpsc::sync_channel(2);
        std::thread::spawn(move || {
            let mut reader = BufReader::new(stdout);
            loop {
                let mut line = vec![];
                let result = reader
                    .by_ref()
                    .take((2 << 20) + 1)
                    .read_until(b'\n', &mut line);
                let value = match result {
                    Ok(0) => break,
                    Ok(_) if line.len() <= 2 << 20 => {
                        serde_json::from_slice(&line).map_err(|e| e.to_string())
                    }
                    _ => Err("media response exceeds framing limit".into()),
                };
                let failed = value.is_err();
                if tx.send(value).is_err() || failed {
                    break;
                }
            }
        });
        let mut player = Self {
            child,
            input,
            responses,
            info: Value::Null,
            poisoned: false,
            _scratch: scratch,
        };
        player.info = player.receive()?;
        if player.info["ready"] != true {
            return Err("media adapter did not become ready".into());
        }
        Ok(player)
    }
    fn receive(&mut self) -> Result<Value, String> {
        let value = self
            .responses
            .recv_timeout(Duration::from_secs(10))
            .map_err(|_| "media adapter timed out/disconnected")??;
        if let Some(error) = value["error"].as_str() {
            return Err(error.into());
        }
        Ok(value)
    }
    pub fn control(&mut self, value: Value) -> Result<Value, String> {
        if self.poisoned {
            return Err("media adapter was closed after a protocol failure".into());
        }
        let line = serde_json::to_vec(&value).map_err(|e| e.to_string())?;
        if line.len() > 8192 {
            return Err("media control exceeds 8 KiB".into());
        }
        let result = self
            .input
            .write_all(&line)
            .and_then(|_| self.input.write_all(b"\n"))
            .and_then(|_| self.input.flush())
            .map_err(|e| e.to_string())
            .and_then(|_| self.receive());
        if result.is_err() {
            self.poisoned = true;
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
        result
    }
    pub fn frame(&mut self) -> Result<Option<Frame>, String> {
        let value = self.control(json!({"command":"frame"}))?;
        let Some(data) = value["bgra"].as_str() else {
            return Ok(None);
        };
        let w = value["width"].as_u64().ok_or("invalid media width")?;
        let h = value["height"].as_u64().ok_or("invalid media height")?;
        if w == 0 || h == 0 || w > 640 || h > 360 {
            return Err("media frame dimensions exceed budget".into());
        }
        let bgra = base64::engine::general_purpose::STANDARD
            .decode(data)
            .map_err(|e| e.to_string())?;
        if bgra.len() as u64 != w * h * 4 {
            return Err("media frame byte count mismatch".into());
        }
        Ok(Some(Frame {
            width: w as u32,
            height: h as u32,
            bgra,
            seconds: value["seconds"].as_f64().ok_or("invalid frame timestamp")?,
        }))
    }
}
impl Drop for Player {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
pub struct Frame {
    pub width: u32,
    pub height: u32,
    pub bgra: Vec<u8>,
    pub seconds: f64,
}

pub fn inspect(path: &std::path::Path) -> Result<Value, String> {
    let extension = path
        .extension()
        .and_then(|x| x.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let mut bytes = vec![];
    std::fs::File::open(path)
        .map_err(|e| e.to_string())?
        .take((64 << 20) + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    let player = Player::new(&bytes, &extension)?;
    Ok(player.info.clone())
}
