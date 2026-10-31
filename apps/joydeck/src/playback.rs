//! Content addressed audiovisual assets plus an owned, kill-on-drop macOS adapter process.
use crate::document::{Deck, MAX_PART, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    io::{BufRead, BufReader, Read, Write},
    path::{Path, PathBuf},
    process::{Child, ChildStdin, Command, Stdio},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Info {
    pub profiles: Vec<String>,
    pub fps: f64,
    pub duration: f64,
    pub codecs: Vec<String>,
    pub width: u32,
    pub height: u32,
    pub adapter: String,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Asset {
    pub extension: String,
    pub info: Info,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub bytes: Vec<u8>,
}
impl Asset {
    pub fn id(&self) -> String {
        format!("{:x}", Sha256::digest(&self.bytes))
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Clip {
    pub id: String,
    pub slide: String,
    pub asset: String,
    pub start: f64,
    pub end: f64,
    pub volume: f32,
    pub looping: bool,
    pub autoplay: bool,
    pub cross_slide: bool,
}
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Playback {
    pub assets: BTreeMap<String, Arc<Asset>>,
    pub clips: Vec<Clip>,
}
impl Playback {
    pub fn validate(&self, d: &Deck) -> Result<()> {
        if self.clips.len() > 64 || self.assets.len() > 64 {
            return Err("媒体数量超过64".into());
        }
        let mut total = 0;
        let mut ids = std::collections::BTreeSet::new();
        for (id, a) in &self.assets {
            total += a.bytes.len();
            if a.bytes.is_empty()
                || a.bytes.len() > MAX_PART
                || &a.id() != id
                || !matches!(a.extension.as_str(), "wav" | "mp3" | "mp4")
                || !a.info.duration.is_finite()
                || a.info.duration <= 0.
                || a.info.duration > 3600.
                || a.info.width > 1920
                || a.info.height > 1080
                || !a.info.fps.is_finite()
                || a.info.fps > 30.01
                || (a.info.width > 0
                    && !a
                        .info
                        .profiles
                        .iter()
                        .all(|s| s == "H264-Baseline-8bit" || s == "H264-Main-8bit"))
            {
                return Err("媒体资源、hash或预算无效".into());
            }
        }
        if total > 32 * 1024 * 1024 {
            return Err("媒体总量超过32MiB".into());
        }
        for c in &self.clips {
            let a = self.assets.get(&c.asset).ok_or("缺少媒体资源")?;
            if (c.cross_slide && a.info.width > 0)
                || c.id.is_empty()
                || c.id.len() > 128
                || !ids.insert(&c.id)
                || !d.slides.iter().any(|s| s.id == c.slide)
                || !c.start.is_finite()
                || !c.end.is_finite()
                || c.start < 0.
                || c.end <= c.start
                || c.end > a.info.duration + 0.01
                || !c.volume.is_finite()
                || !(0. ..=1.).contains(&c.volume)
            {
                return Err("媒体参数或锚点无效".into());
            }
        }
        Ok(())
    }
}
pub fn reconcile(d: &mut Deck) {
    d.playback
        .clips
        .retain(|c| d.slides.iter().any(|s| s.id == c.slide));
    d.playback
        .assets
        .retain(|id, _| d.playback.clips.iter().any(|c| &c.asset == id));
}
pub fn helper() -> Result<PathBuf> {
    if !cfg!(target_os = "macos") {
        return Err("本构建尚无该平台媒体adapter".into());
    }
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    for p in [
        exe.parent().unwrap().join("joydeck-media-macos"),
        PathBuf::from(env!("OUT_DIR")).join("joydeck-media-macos"),
        exe.parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("joydeck-media-macos"),
    ] {
        if p.is_file() {
            return Ok(p);
        }
    }
    Err("缺少随包提供的joydeck-media-macos适配器".into())
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
        crate::storage::private_directory("media").map(Self)
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
                let mut s = shared.lock().unwrap();
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
                    let w = u32::from_le_bytes(bytes[0..4].try_into().unwrap());
                    let h = u32::from_le_bytes(bytes[4..8].try_into().unwrap());
                    let g = u32::from_le_bytes(bytes[8..12].try_into().unwrap());
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
            let mut s = shared.lock().unwrap();
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
            let mut s = self.status.lock().unwrap();
            s.requested_generation = s.requested_generation.max(s.generation) + 1;
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
