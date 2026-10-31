//! Deterministic static-page MP4 export with a bounded stream to a platform encoder.
use crate::document::{Deck, MAX_FILE, Result, check_cancel};
use serde::{Deserialize, Serialize};
use std::{
    io::{Read, Write},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{
        atomic::AtomicBool,
        mpsc::{self, SyncSender, TrySendError},
    },
    time::{Duration, Instant},
};
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Options {
    pub width: u32,
    pub fps: u32,
    pub seconds_per_page: u32,
}
impl Default for Options {
    fn default() -> Self {
        Self {
            width: 960,
            fps: 15,
            seconds_per_page: 2,
        }
    }
}
#[derive(Clone, Debug, Serialize)]
pub struct Plan {
    pub width: u32,
    pub height: u32,
    pub frames: u32,
    pub fps: u32,
    pub duration_seconds: u32,
    pub scope: &'static str,
}
pub fn plan(d: &Deck, pages: &[usize], options: &Options) -> Result<Plan> {
    d.validate()?;
    let height =
        ((options.width as f64 * d.height as f64 / d.width as f64 / 2.).round() * 2.) as u32;
    let frames = pages.len() as u64 * options.fps as u64 * options.seconds_per_page as u64;
    if !(320..=1920).contains(&options.width)
        || !options.width.is_multiple_of(2)
        || !(2..=1080).contains(&height)
        || !(1..=30).contains(&options.fps)
        || !(1..=30).contains(&options.seconds_per_page)
        || pages.is_empty()
        || pages.iter().any(|&p| p >= d.slides.len())
        || frames > 1800
        || frames == 0
    {
        return Err(
            "MP4参数超出预算：偶数宽320–1920、高≤1080、1–30fps、每页1–30秒、总计≤1800帧".into(),
        );
    }
    Ok(Plan {
        width: options.width,
        height,
        frames: frames as u32,
        fps: options.fps,
        duration_seconds: pages.len() as u32 * options.seconds_per_page,
        scope: "static pages; silent H.264 Main MP4; no transitions, animation, embedded media or recording",
    })
}
/// A sidecar of page-title cues using exactly the static video page durations. Speaker notes stay private.
pub fn captions(d: &Deck, pages: &[usize], options: &Options) -> Result<String> {
    plan(d, pages, options)?;
    let stamp = |seconds: u32| {
        format!(
            "{:02}:{:02}:{:02}.000",
            seconds / 3600,
            seconds / 60 % 60,
            seconds % 60
        )
    };
    let mut text = String::from("WEBVTT\n\n");
    for (i, &page) in pages.iter().enumerate() {
        let title = d.slides[page]
            .title
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
            .replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;");
        text.push_str(&format!(
            "{}\n{} --> {}\n{}\n\n",
            i + 1,
            stamp(i as u32 * options.seconds_per_page),
            stamp((i + 1) as u32 * options.seconds_per_page),
            title
        ));
    }
    Ok(text)
}
fn helper() -> Result<PathBuf> {
    if !cfg!(target_os = "macos") {
        return Err("MP4编码适配器仅在macOS可用".into());
    }
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    [
        exe.parent().unwrap().join("joydeck-video-macos"),
        PathBuf::from(env!("OUT_DIR")).join("joydeck-video-macos"),
        exe.parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("joydeck-video-macos"),
    ]
    .into_iter()
    .find(|p| p.is_file())
    .ok_or("缺少MP4编码适配器".into())
}
struct Scratch(PathBuf);
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
struct Encoder {
    child: Child,
    send: Option<SyncSender<Vec<u8>>>,
    writer: Option<std::thread::JoinHandle<std::io::Result<()>>>,
}
impl Drop for Encoder {
    fn drop(&mut self) {
        self.send.take();
        let _ = self.child.kill();
        let _ = self.child.wait();
        if let Some(w) = self.writer.take() {
            let _ = w.join();
        }
    }
}
pub fn export(
    path: &Path,
    d: &Deck,
    pages: &[usize],
    options: &Options,
    cancel: &AtomicBool,
) -> Result<Plan> {
    check_cancel(cancel)?;
    let plan = plan(d, pages, options)?;
    let helper = helper()?;
    let dir = crate::storage::private_directory("video")?;
    let scratch = Scratch(dir);
    let encoded = scratch.0.join("encoded.mp4");
    let mut child = Command::new(helper)
        .arg(&encoded)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| e.to_string())?;
    let mut stdin = child.stdin.take().ok_or("缺少编码输入")?;
    let (tx, rx) = mpsc::sync_channel::<Vec<u8>>(1);
    let header = [plan.width, plan.height, plan.fps, plan.frames]
        .into_iter()
        .flat_map(u32::to_le_bytes)
        .collect::<Vec<_>>();
    let writer = std::thread::spawn(move || {
        stdin.write_all(&header)?;
        for bytes in rx {
            stdin.write_all(&bytes)?;
        }
        stdin.flush()
    });
    let mut encoder = Encoder {
        child,
        send: Some(tx),
        writer: Some(writer),
    };
    let start = Instant::now();
    let bounded = |cancel: &AtomicBool| -> Result<()> {
        check_cancel(cancel)?;
        if start.elapsed() > Duration::from_secs(600) {
            Err("MP4编码超过10分钟预算".into())
        } else {
            Ok(())
        }
    };
    for &page in pages {
        bounded(cancel)?;
        let frame = crate::preview::render(d, page, plan.width, cancel)?;
        let bytes = if frame.height == plan.height {
            frame.bgra
        } else {
            image::imageops::resize(
                &image::RgbaImage::from_raw(frame.width, frame.height, frame.bgra)
                    .ok_or("无效帧")?,
                plan.width,
                plan.height,
                image::imageops::FilterType::Triangle,
            )
            .into_raw()
        };
        for _ in 0..options.fps * options.seconds_per_page {
            let mut pending = bytes.clone();
            loop {
                bounded(cancel)?;
                match encoder.send.as_ref().unwrap().try_send(pending) {
                    Ok(()) => break,
                    Err(TrySendError::Full(bytes)) => {
                        pending = bytes;
                        std::thread::sleep(Duration::from_millis(10));
                    }
                    Err(TrySendError::Disconnected(_)) => return Err("MP4编码进程中断".into()),
                }
            }
        }
    }
    encoder.send.take();
    loop {
        bounded(cancel)?;
        if let Some(status) = encoder.child.try_wait().map_err(|e| e.to_string())? {
            if !status.success() {
                return Err("系统MP4编码失败".into());
            }
            break;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    encoder
        .writer
        .take()
        .unwrap()
        .join()
        .map_err(|_| "MP4输入线程异常")?
        .map_err(|e| e.to_string())?;
    let mut bytes = vec![];
    std::fs::File::open(&encoded)
        .map_err(|e| e.to_string())?
        .take(MAX_FILE + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    crate::export::atomic(path, &bytes, None, cancel)?;
    Ok(plan)
}
