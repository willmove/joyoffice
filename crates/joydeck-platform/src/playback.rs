//! Content addressed audiovisual assets plus an owned, kill-on-drop macOS adapter process.
use joydeck_model::document::{MAX_PART, Result};
pub use joydeck_model::playback::*;
use std::{
    io::{BufRead, BufReader, Read, Write},
    path::{Path, PathBuf},
    process::{Child, ChildStdin, Command, Stdio},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
pub fn helper() -> Result<PathBuf> {
    if !cfg!(target_os = "macos") {
        return Err("本构建尚无该平台媒体adapter".into());
    }
    crate::helpers::find("media")
}

pub fn import(path: &Path) -> Result<Asset> {
    let extension = path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if !matches!(extension.as_str(), "wav" | "mp3" | "mp4") {
        return Err("只支持PCM/WAV、MP3、H.264 MP4".into());
    }
    let mut bytes = vec![];
    std::fs::File::open(path)
        .map_err(|e| e.to_string())?
        .take(MAX_PART as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > MAX_PART || bytes.is_empty() {
        return Err("媒体必须在1字节–8MiB之间".into());
    }
    let scratch = Scratch::new()?;
    let file = scratch.0.join(format!("input.{extension}"));
    std::fs::write(&file, &bytes).map_err(|e| e.to_string())?;
    let output = Command::new(helper()?)
        .arg("probe")
        .arg(&file)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| e.to_string())?;
    let mut child = output;
    let start = Instant::now();
    while child.try_wait().map_err(|e| e.to_string())?.is_none() {
        if start.elapsed() > Duration::from_secs(10) {
            let _ = child.kill();
            let _ = child.wait();
            return Err("媒体探测超时".into());
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    let out = child.wait_with_output().map_err(|e| e.to_string())?;
    if !out.status.success() {
        return Err(format!(
            "不支持或损坏的媒体：{}",
            String::from_utf8_lossy(&out.stdout)
        ));
    }
    let info: Info = serde_json::from_slice(&out.stdout).map_err(|e| e.to_string())?;
    Ok(Asset {
        extension,
        info,
        bytes,
    })
}
struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Result<Self> {
        crate::files::private_directory("media").map(Self)
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
#[derive(Clone, Default)]
pub struct Status {
    pub seconds: f64,
    pub playing: bool,
    pub ready: bool,
    pub ended: bool,
    pub generation: u64,
    pub requested_generation: u64,
    pub sequence: u64,
    pub frame: Option<Arc<Frame>>,
    pub error: Option<String>,
}
pub struct Frame {
    pub width: u32,
    pub height: u32,
    pub bgra: Vec<u8>,
}
pub struct Player {
    child: Child,
    input: ChildStdin,
    pub status: Arc<Mutex<Status>>,
    reader: Option<std::thread::JoinHandle<()>>,
    _scratch: Scratch,
}
impl Player {
    pub fn new(asset: &Asset, c: &Clip) -> Result<Self> {
        let scratch = Scratch::new()?;
        let file = scratch.0.join(format!("input.{}", asset.extension));
        std::fs::write(&file, &asset.bytes).map_err(|e| e.to_string())?;
        let mut child = Command::new(helper()?)
            .arg("play")
            .arg(file)
            .arg(&scratch.0)
            .args([
                c.start.to_string(),
                c.end.to_string(),
                c.volume.to_string(),
                c.looping.to_string(),
                c.autoplay.to_string(),
            ])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| e.to_string())?;
        let input = child.stdin.take().ok_or("缺少媒体stdin")?;
        let stdout = child.stdout.take().ok_or("缺少媒体stdout")?;
        let status = Arc::new(Mutex::new(Status::default()));
        let shared = status.clone();
        let frame_file = scratch.0.join("frame.bgra");
        let reader = std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines() {
                let Ok(line) = line else { break };
                if line.len() > 8192 {
                    break;
                }
                let Ok(v) = serde_json::from_str::<serde_json::Value>(&line) else {
                    continue;
                };
                let Ok(mut s) = shared.lock() else {
                    return;
                };
                if let Some(e) = v["error"].as_str() {
                    s.error = Some(e.into());
                    break;
                }
                let incoming = v["generation"].as_u64().unwrap_or(0);
                if incoming < s.requested_generation {
                    continue;
                }
                s.seconds = v["seconds"].as_f64().unwrap_or(0.);
                s.ready = v["ready"] == true;
                s.playing = v["playing"] == true;
                s.ended = v["ended"] == true;
                let generation = v["generation"].as_u64().unwrap_or(0);
                if generation != s.generation {
                    s.frame = None;
                }
                s.generation = generation;
                s.sequence = v["sequence"].as_u64().unwrap_or(0);
                if v["frame"] == true
                    && let Ok(bytes) = std::fs::read(&frame_file)
                    && bytes.len() >= 12
                {
                    let w = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
                    let h = u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);
                    let g = u32::from_le_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]);
                    if u64::from(w) * u64::from(h) <= 960 * 960
                        && bytes.len() == 12 + w as usize * h as usize * 4
                        && u64::from(g) == s.generation
                    {
                        s.frame = Some(Arc::new(Frame {
                            width: w,
                            height: h,
                            bgra: bytes[12..].to_vec(),
                        }));
                    }
                }
            }
            let Ok(mut s) = shared.lock() else {
                return;
            };
            s.playing = false;
            if !s.ended && s.error.is_none() {
                s.error = Some("媒体适配器已关闭".into());
            }
        });
        Ok(Self {
            child,
            input,
            status,
            reader: Some(reader),
            _scratch: scratch,
        })
    }
    pub fn control(&mut self, value: serde_json::Value) -> Result<()> {
        if value["command"] == "seek" {
            let mut s = self.status.lock().map_err(|_| "媒体状态已损坏，拒绝控制")?;
            s.requested_generation = s
                .requested_generation
                .max(s.generation)
                .checked_add(1)
                .ok_or("媒体代次已耗尽")?;
            s.frame = None;
        }
        writeln!(self.input, "{value}").map_err(|e| e.to_string())?;
        self.input.flush().map_err(|e| e.to_string())
    }
}
impl Drop for Player {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        if let Some(t) = self.reader.take() {
            let _ = t.join();
        }
    }
}
