//! Private stdio media protocol, kill-on-drop ownership and bounded native frame delivery.
use crate::helpers::{self, Scratch};
use base64::Engine;
use serde_json::{Value, json};
use std::{
    io::{BufRead, BufReader, Read, Write},
    path::{Path, PathBuf},
    process::{Child, ChildStdin, Command, Stdio},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver},
    },
    thread::JoinHandle,
    time::{Duration, Instant},
};

const MAX_MEDIA_BYTES: usize = 64 << 20;
const MAX_DURATION_SECS: f64 = 3600.0;
const MAX_VIDEO_SIDE: u32 = 8192;
const MAX_VIDEO_PIXELS: u64 = 16_000_000;
const MAX_FRAME_W: u32 = 640;
const MAX_FRAME_H: u32 = 360;
const VIDEO_FPS: f64 = 24.0;
const FFPROBE_MAX_BYTES: u64 = 256 << 10;

pub struct Player {
    backend: Backend,
    pub info: Value,
    _scratch: Scratch,
}

enum Backend {
    Helper(HelperBackend),
    Ffmpeg(Box<FfmpegBackend>),
}

struct HelperBackend {
    child: Child,
    input: ChildStdin,
    responses: Receiver<Result<Value, String>>,
    poisoned: bool,
}

impl HelperBackend {
    fn new(helper: PathBuf, file: &Path) -> Result<(Self, Value), String> {
        let mut child = helpers::command(helper)
            .arg(helpers::native_path(file)?)
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
        let mut backend = Self {
            child,
            input,
            responses,
            poisoned: false,
        };
        let info = backend.receive()?;
        if info["ready"] != true {
            return Err("media adapter did not become ready".into());
        }
        Ok((backend, info))
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

    fn control(&mut self, value: Value) -> Result<Value, String> {
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

    fn frame(&mut self) -> Result<Option<Frame>, String> {
        let value = self.control(json!({"command":"frame"}))?;
        let Some(data) = value["bgra"].as_str() else {
            return Ok(None);
        };
        let w = value["width"].as_u64().ok_or("invalid media width")?;
        let h = value["height"].as_u64().ok_or("invalid media height")?;
        if w == 0 || h == 0 || w > u64::from(MAX_FRAME_W) || h > u64::from(MAX_FRAME_H) {
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

impl Drop for HelperBackend {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[derive(Default)]
struct VideoSlot {
    latest: Option<Frame>,
    eof: bool,
}

struct VideoWorker {
    child: Child,
    stop: Arc<AtomicBool>,
    reader: Option<JoinHandle<()>>,
}

impl VideoWorker {
    fn spawn(
        ffmpeg: &Path,
        file: &Path,
        start_pos: f64,
        out_w: u32,
        out_h: u32,
        slot: Arc<Mutex<VideoSlot>>,
    ) -> Result<Self, String> {
        let mut cmd = Command::new(ffmpeg);
        cmd.arg("-nostdin")
            .arg("-hide_banner")
            .arg("-loglevel")
            .arg("error");
        if start_pos > 0.001 {
            cmd.arg("-ss").arg(format!("{start_pos:.3}"));
        }
        cmd.arg("-i")
            .arg(file)
            .arg("-an")
            .arg("-sn")
            .arg("-dn")
            .arg("-vf")
            .arg(format!(
                "fps={VIDEO_FPS:.0},scale={out_w}:{out_h}:flags=fast_bilinear"
            ))
            .arg("-f")
            .arg("rawvideo")
            .arg("-pix_fmt")
            .arg("bgra")
            .arg("pipe:1")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        let mut child = cmd
            .spawn()
            .map_err(|e| format!("ffmpeg video spawn failed: {e}"))?;
        let mut stdout = child.stdout.take().ok_or("missing ffmpeg video stdout")?;
        let stop = Arc::new(AtomicBool::new(false));
        let stop_flag = stop.clone();
        let frame_bytes = (out_w as usize)
            .checked_mul(out_h as usize)
            .and_then(|n| n.checked_mul(4))
            .ok_or("media frame byte size overflow")?;
        let reader = std::thread::spawn(move || {
            let started = Instant::now();
            let mut frame_index: u64 = 0;
            loop {
                if stop_flag.load(Ordering::Acquire) {
                    break;
                }
                let mut bgra = vec![0u8; frame_bytes];
                if stdout.read_exact(&mut bgra).is_err() {
                    if !stop_flag.load(Ordering::Acquire)
                        && let Ok(mut guard) = slot.lock()
                    {
                        guard.eof = true;
                    }
                    break;
                }
                let rel_pts = (frame_index as f64) / VIDEO_FPS;
                if frame_index > 0 {
                    while !stop_flag.load(Ordering::Acquire) {
                        let elapsed = started.elapsed().as_secs_f64();
                        let remaining = rel_pts - elapsed;
                        if remaining <= 0.005 {
                            break;
                        }
                        std::thread::sleep(Duration::from_millis(
                            (remaining * 1000.0).clamp(1.0, 15.0) as u64,
                        ));
                    }
                    if stop_flag.load(Ordering::Acquire) {
                        break;
                    }
                }
                let seconds = start_pos + rel_pts;
                if let Ok(mut guard) = slot.lock() {
                    guard.latest = Some(Frame {
                        width: out_w,
                        height: out_h,
                        bgra,
                        seconds,
                    });
                }
                frame_index = frame_index.saturating_add(1);
            }
        });
        Ok(Self {
            child,
            stop,
            reader: Some(reader),
        })
    }

    fn stop(&mut self) {
        self.stop.store(true, Ordering::Release);
        let _ = self.child.kill();
        let _ = self.child.wait();
        if let Some(handle) = self.reader.take() {
            let _ = handle.join();
        }
    }
}

impl Drop for VideoWorker {
    fn drop(&mut self) {
        self.stop();
    }
}

enum AudioSinkKind {
    PwPlay(PathBuf),
    Paplay(PathBuf),
    Aplay(PathBuf),
}

struct AudioWorker {
    ffmpeg: Child,
    sink: Child,
}

impl AudioWorker {
    fn spawn(ffmpeg_bin: &Path, file: &Path, start_pos: f64, volume: f64) -> Option<Self> {
        let sink_kind = find_audio_sink()?;
        let mut dec = Command::new(ffmpeg_bin);
        dec.arg("-nostdin")
            .arg("-hide_banner")
            .arg("-loglevel")
            .arg("error");
        if start_pos > 0.001 {
            dec.arg("-ss").arg(format!("{start_pos:.3}"));
        }
        dec.arg("-i")
            .arg(file)
            .arg("-vn")
            .arg("-sn")
            .arg("-dn")
            .arg("-af")
            .arg(format!("volume={:.3}", volume.clamp(0.0, 1.0)))
            .arg("-f")
            .arg("s16le")
            .arg("-ac")
            .arg("2")
            .arg("-ar")
            .arg("48000")
            .arg("pipe:1")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        let mut ffmpeg = dec.spawn().ok()?;
        let Some(pcm_out) = ffmpeg.stdout.take() else {
            let _ = ffmpeg.kill();
            let _ = ffmpeg.wait();
            return None;
        };
        let mut sink_cmd = match sink_kind {
            AudioSinkKind::PwPlay(path) => {
                let mut c = Command::new(path);
                c.args(["--raw", "--format=s16", "--rate=48000", "--channels=2", "-"]);
                c
            }
            AudioSinkKind::Paplay(path) => {
                let mut c = Command::new(path);
                c.args(["--raw", "--format=s16le", "--rate=48000", "--channels=2"]);
                c
            }
            AudioSinkKind::Aplay(path) => {
                let mut c = Command::new(path);
                c.args(["-q", "-f", "S16_LE", "-r", "48000", "-c", "2", "-"]);
                c
            }
        };
        sink_cmd
            .stdin(Stdio::from(pcm_out))
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        match sink_cmd.spawn() {
            Ok(sink) => Some(Self { ffmpeg, sink }),
            Err(_) => {
                let _ = ffmpeg.kill();
                let _ = ffmpeg.wait();
                None
            }
        }
    }

    fn stop(&mut self) {
        let _ = self.sink.kill();
        let _ = self.ffmpeg.kill();
        let _ = self.sink.wait();
        let _ = self.ffmpeg.wait();
    }
}

impl Drop for AudioWorker {
    fn drop(&mut self) {
        self.stop();
    }
}

struct FfmpegBackend {
    ffmpeg: PathBuf,
    file: PathBuf,
    duration: f64,
    duration_known: bool,
    has_video: bool,
    has_audio: bool,
    out_w: u32,
    out_h: u32,
    paused: bool,
    base_pts: f64,
    started_at: Option<Instant>,
    volume: f64,
    slot: Arc<Mutex<VideoSlot>>,
    video: Option<VideoWorker>,
    audio: Option<AudioWorker>,
}

impl FfmpegBackend {
    fn new(file: PathBuf) -> Result<(Self, Value), String> {
        let ffprobe = find_tool("ffprobe")
            .ok_or_else(|| "media playback requires ffprobe on this platform".to_string())?;
        let ffmpeg = find_tool("ffmpeg")
            .ok_or_else(|| "media playback requires ffmpeg on this platform".to_string())?;
        let probe = run_ffprobe(&ffprobe, &file)?;

        let streams = probe["streams"]
            .as_array()
            .ok_or("media probe returned no streams")?;
        let mut src_w: u32 = 0;
        let mut src_h: u32 = 0;
        let mut has_video = false;
        let mut has_audio = false;
        let mut stream_duration: Option<f64> = None;

        for stream in streams {
            let kind = stream["codec_type"].as_str().unwrap_or("");
            if kind == "video" && !has_video {
                let attached = stream["disposition"]["attached_pic"].as_i64().unwrap_or(0) != 0;
                if !attached {
                    let w = stream["width"].as_u64().unwrap_or(0);
                    let h = stream["height"].as_u64().unwrap_or(0);
                    if w > 0 && h > 0 {
                        if w > u64::from(MAX_VIDEO_SIDE)
                            || h > u64::from(MAX_VIDEO_SIDE)
                            || w.saturating_mul(h) > MAX_VIDEO_PIXELS
                        {
                            return Err("media video dimensions exceed budget".into());
                        }
                        src_w = w as u32;
                        src_h = h as u32;
                        has_video = true;
                    }
                }
            } else if kind == "audio" {
                has_audio = true;
            }
            if stream_duration.is_none()
                && let Some(d) = parse_duration(&stream["duration"])
            {
                stream_duration = Some(d);
            }
        }
        if !has_video && !has_audio {
            return Err("media file has no playable audio or video stream".into());
        }
        let duration_opt = parse_duration(&probe["format"]["duration"]).or(stream_duration);
        let duration_known = duration_opt.is_some();
        let duration = duration_opt.unwrap_or(0.0);
        if duration > MAX_DURATION_SECS {
            return Err("media duration exceeds 3600 seconds budget".into());
        }
        let (out_w, out_h) = if has_video {
            fit_frame_size(src_w, src_h)
        } else {
            (0, 0)
        };
        let info = json!({
            "ready": true,
            "width": src_w,
            "height": src_h,
            "duration": duration,
            "durationKnown": duration_known,
            "audio": has_audio,
            "video": has_video,
            "backend": "Linux FFmpeg"
        });
        let backend = Self {
            ffmpeg,
            file,
            duration,
            duration_known,
            has_video,
            has_audio,
            out_w,
            out_h,
            paused: true,
            base_pts: 0.0,
            started_at: None,
            volume: 0.0,
            slot: Arc::new(Mutex::new(VideoSlot::default())),
            video: None,
            audio: None,
        };
        Ok((backend, info))
    }

    fn current_seconds(&self) -> f64 {
        let raw = if !self.paused
            && let Some(started) = self.started_at
        {
            self.base_pts + started.elapsed().as_secs_f64()
        } else {
            self.base_pts
        };
        let limit = if self.duration_known && self.duration > 0.0 {
            self.duration
        } else {
            MAX_DURATION_SECS
        };
        raw.clamp(0.0, limit)
    }

    fn stop_workers(&mut self) {
        self.video = None;
        self.audio = None;
    }

    fn start_workers(&mut self) -> Result<(), String> {
        self.stop_workers();
        let pos = self.base_pts;
        if self.has_video {
            if let Ok(mut guard) = self.slot.lock() {
                guard.eof = false;
            }
            self.video = Some(VideoWorker::spawn(
                &self.ffmpeg,
                &self.file,
                pos,
                self.out_w,
                self.out_h,
                self.slot.clone(),
            )?);
        }
        if self.has_audio && self.volume > 0.001 {
            self.audio = AudioWorker::spawn(&self.ffmpeg, &self.file, pos, self.volume);
        }
        Ok(())
    }

    fn check_ended(&mut self) {
        if self.paused {
            return;
        }
        let pos = self.current_seconds();
        let reached_duration = self.duration_known && self.duration > 0.0 && pos >= self.duration;
        let video_eof = self.has_video
            && self.slot.lock().is_ok_and(|g| g.eof)
            && (!self.duration_known || pos + 0.25 >= self.duration);
        if reached_duration || video_eof {
            self.base_pts = if self.duration_known && self.duration > 0.0 {
                self.duration
            } else {
                pos
            };
            self.paused = true;
            self.started_at = None;
            self.stop_workers();
        }
    }

    fn status_json(&self) -> Value {
        json!({
            "ready": true,
            "seconds": self.current_seconds(),
            "paused": self.paused,
            "duration": self.duration,
            "playing": !self.paused,
        })
    }

    fn decode_single_frame(&self, at: f64) -> Option<Frame> {
        if !self.has_video || self.out_w == 0 || self.out_h == 0 {
            return None;
        }
        let frame_bytes = (self.out_w as usize)
            .checked_mul(self.out_h as usize)
            .and_then(|n| n.checked_mul(4))?;
        let mut cmd = Command::new(&self.ffmpeg);
        cmd.arg("-nostdin")
            .arg("-hide_banner")
            .arg("-loglevel")
            .arg("error");
        if at > 0.001 {
            cmd.arg("-ss").arg(format!("{at:.3}"));
        }
        cmd.arg("-i")
            .arg(&self.file)
            .arg("-an")
            .arg("-sn")
            .arg("-dn")
            .arg("-frames:v")
            .arg("1")
            .arg("-vf")
            .arg(format!(
                "scale={}:{}:flags=fast_bilinear",
                self.out_w, self.out_h
            ))
            .arg("-f")
            .arg("rawvideo")
            .arg("-pix_fmt")
            .arg("bgra")
            .arg("pipe:1")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        let mut child = cmd.spawn().ok()?;
        let mut stdout = child.stdout.take()?;
        let mut bgra = vec![0u8; frame_bytes];
        let ok = stdout.read_exact(&mut bgra).is_ok();
        let _ = child.kill();
        let _ = child.wait();
        ok.then_some(Frame {
            width: self.out_w,
            height: self.out_h,
            bgra,
            seconds: at,
        })
    }

    fn control(&mut self, value: Value) -> Result<Value, String> {
        self.check_ended();
        let cmd = value["command"]
            .as_str()
            .ok_or("missing media control command")?;
        match cmd {
            "play" => {
                if self.duration_known
                    && self.duration > 0.0
                    && self.base_pts + 0.02 >= self.duration
                {
                    self.base_pts = 0.0;
                }
                if self.paused {
                    self.paused = false;
                    self.started_at = Some(Instant::now());
                    self.start_workers()?;
                }
            }
            "pause" => {
                if !self.paused {
                    self.base_pts = self.current_seconds();
                    self.paused = true;
                    self.started_at = None;
                    self.stop_workers();
                }
            }
            "seek" => {
                let at = value["seconds"].as_f64().ok_or("invalid seek seconds")?;
                let limit = if self.duration_known && self.duration > 0.0 {
                    self.duration
                } else {
                    MAX_DURATION_SECS
                };
                if !at.is_finite() || !(0.0..=MAX_DURATION_SECS).contains(&at) {
                    return Err("media seek exceeds range".into());
                }
                self.base_pts = at.min(limit);
                if self.paused {
                    self.stop_workers();
                    if let Some(frame) = self.decode_single_frame(self.base_pts)
                        && let Ok(mut guard) = self.slot.lock()
                    {
                        guard.latest = Some(frame);
                        guard.eof = false;
                    }
                } else {
                    self.started_at = Some(Instant::now());
                    self.start_workers()?;
                }
            }
            "volume" => {
                let v = value["value"].as_f64().ok_or("invalid volume value")?;
                if !v.is_finite() {
                    return Err("invalid volume value".into());
                }
                let clamped = v.clamp(0.0, 1.0);
                let changed = (clamped - self.volume).abs() > 0.005;
                self.volume = clamped;
                if changed && !self.paused && self.has_audio {
                    self.audio = None;
                    if self.volume > 0.001 {
                        let pos = self.current_seconds();
                        self.audio = AudioWorker::spawn(&self.ffmpeg, &self.file, pos, self.volume);
                    }
                }
            }
            "status" => {}
            _ => return Err(format!("unsupported media command: {cmd}")),
        }
        Ok(self.status_json())
    }

    fn frame(&mut self) -> Result<Option<Frame>, String> {
        self.check_ended();
        if !self.has_video {
            return Ok(None);
        }
        let cached = self
            .slot
            .lock()
            .map_err(|_| "video slot lock poisoned".to_string())?
            .latest
            .clone();
        if cached.is_some() {
            return Ok(cached);
        }
        if let Some(frame) = self.decode_single_frame(self.current_seconds()) {
            if let Ok(mut guard) = self.slot.lock() {
                guard.latest = Some(frame.clone());
            }
            return Ok(Some(frame));
        }
        Ok(None)
    }
}

impl Drop for FfmpegBackend {
    fn drop(&mut self) {
        self.stop_workers();
    }
}

impl Player {
    pub fn new(bytes: &[u8], extension: &str) -> Result<Self, String> {
        if bytes.is_empty()
            || bytes.len() > MAX_MEDIA_BYTES
            || !["mp4", "mov", "wmv", "mp3", "wav", "wma", "m4a"].contains(&extension)
        {
            return Err("unsupported/oversize local media".into());
        }
        let scratch = Scratch::new()?;
        let file = scratch.0.join(format!("media.{extension}"));
        std::fs::write(&file, bytes).map_err(|e| e.to_string())?;
        if let Ok(helper) = helpers::find("media") {
            let (backend, info) = HelperBackend::new(helper, &file)?;
            return Ok(Self {
                backend: Backend::Helper(backend),
                info,
                _scratch: scratch,
            });
        }
        let (backend, info) = FfmpegBackend::new(file)?;
        Ok(Self {
            backend: Backend::Ffmpeg(Box::new(backend)),
            info,
            _scratch: scratch,
        })
    }

    pub fn control(&mut self, value: Value) -> Result<Value, String> {
        match &mut self.backend {
            Backend::Helper(h) => h.control(value),
            Backend::Ffmpeg(f) => f.control(value),
        }
    }

    pub fn frame(&mut self) -> Result<Option<Frame>, String> {
        match &mut self.backend {
            Backend::Helper(h) => h.frame(),
            Backend::Ffmpeg(f) => f.frame(),
        }
    }
}

#[derive(Clone)]
pub struct Frame {
    pub width: u32,
    pub height: u32,
    pub bgra: Vec<u8>,
    pub seconds: f64,
}

pub fn inspect(path: &Path) -> Result<Value, String> {
    let extension = path
        .extension()
        .and_then(|x| x.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let mut bytes = vec![];
    std::fs::File::open(path)
        .map_err(|e| e.to_string())?
        .take((MAX_MEDIA_BYTES as u64) + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    let player = Player::new(&bytes, &extension)?;
    Ok(player.info.clone())
}

fn find_tool(name: &str) -> Option<PathBuf> {
    if let Some(paths) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&paths) {
            let candidate = dir.join(name);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    for dir in ["/usr/bin", "/usr/local/bin", "/bin"] {
        let candidate = Path::new(dir).join(name);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

fn find_audio_sink() -> Option<AudioSinkKind> {
    if let Some(p) = find_tool("pw-play") {
        return Some(AudioSinkKind::PwPlay(p));
    }
    if let Some(p) = find_tool("paplay") {
        return Some(AudioSinkKind::Paplay(p));
    }
    if let Some(p) = find_tool("aplay") {
        return Some(AudioSinkKind::Aplay(p));
    }
    None
}

fn run_ffprobe(ffprobe: &Path, file: &Path) -> Result<Value, String> {
    let mut child = Command::new(ffprobe)
        .args([
            "-v",
            "error",
            "-print_format",
            "json",
            "-show_format",
            "-show_streams",
            "--",
        ])
        .arg(file)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("ffprobe spawn failed: {e}"))?;
    let stdout = child.stdout.take().ok_or("missing ffprobe output")?;
    let (tx, rx) = mpsc::sync_channel(1);
    std::thread::spawn(move || {
        let mut buf = Vec::new();
        let res = stdout
            .take(FFPROBE_MAX_BYTES + 1)
            .read_to_end(&mut buf)
            .map_err(|e| e.to_string())
            .and_then(|_| {
                if buf.len() as u64 > FFPROBE_MAX_BYTES {
                    Err("ffprobe output exceeds budget".into())
                } else {
                    Ok(buf)
                }
            });
        let _ = tx.send(res);
    });
    let bytes = match rx.recv_timeout(Duration::from_secs(10)) {
        Ok(res) => {
            let _ = child.wait();
            res?
        }
        Err(_) => {
            let _ = child.kill();
            let _ = child.wait();
            return Err("ffprobe timed out".into());
        }
    };
    serde_json::from_slice(&bytes).map_err(|e| format!("invalid ffprobe metadata: {e}"))
}

fn parse_duration(v: &Value) -> Option<f64> {
    let parsed = v
        .as_f64()
        .or_else(|| v.as_str().and_then(|s| s.parse::<f64>().ok()))?;
    (parsed.is_finite() && parsed >= 0.0).then_some(parsed)
}

fn fit_frame_size(src_w: u32, src_h: u32) -> (u32, u32) {
    let k = 1.0_f64
        .min(f64::from(MAX_FRAME_W) / f64::from(src_w.max(1)))
        .min(f64::from(MAX_FRAME_H) / f64::from(src_h.max(1)));
    let mut w = ((f64::from(src_w) * k).round() as u32).clamp(2, MAX_FRAME_W);
    let mut h = ((f64::from(src_h) * k).round() as u32).clamp(2, MAX_FRAME_H);
    if w > 2 && w % 2 == 1 {
        w -= 1;
    }
    if h > 2 && h % 2 == 1 {
        h -= 1;
    }
    (w, h)
}
